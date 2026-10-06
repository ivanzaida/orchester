//! Auto mode in the worker (`orchester_core::run::review`): a request the rules leave to a person is
//! shown and waits as always, and meanwhile, when the run uses auto mode, the reviewer judges it on the
//! blocking pool, so the worker keeps following its agent. An approval answers the request as the
//! reviewer's; anything else leaves it to the person with what auto mode said. A person who answers
//! first wins, and the review that comes later is dropped.

use std::time::Duration;

use orchester_core::{
  connection::{
    invocation::agent_environment,
    reviewer::{ClaudeReviewer, reviewer_connection},
  },
  permission::AgentPermissions,
  run::{
    ApprovalSubject, DecidedBy, PendingToolApproval,
    review::{
      RequestReviewer, Review, ReviewFailure, ReviewInput, ReviewedRequest, ReviewedStep, ReviewedTask, review_request,
    },
  },
  store::{ConnectionStore, RunStore, SettingsStore, TaskStore},
};
use orchester_exec::ProcessRunner;
use orchester_ipc::Event;
use orchester_store::StoreError;
use tokio::task::JoinHandle;

use super::{super::answer::Settled, Watch, Worker};

/// The longest a review may take; the reviewer is stopped then, and the person decides.
const REVIEW_TIMEOUT: Duration = Duration::from_secs(60);
/// How many of the agent's last messages the reviewer reads.
const MESSAGES_READ: usize = 3;

/// A review under way: the request it is about, and the work that reviews it.
pub(in super::super) struct Reviewing {
  approval: String,
  work: JoinHandle<Review>,
}

/// Ready once the review under way in `slot` ends: the request, and what auto mode did with it.
pub(super) async fn next(slot: &mut Option<Reviewing>) -> (String, Review) {
  let Some(reviewing) = slot.as_mut() else {
    return std::future::pending().await;
  };
  let review = (&mut reviewing.work).await.unwrap_or_else(|error| {
    tracing::error!(%error, "a review did not finish");
    Review::Failed {
      failure: ReviewFailure::CouldNotRun,
      model: None,
    }
  });
  let reviewing = slot.take().expect("the review awaited above is under way");
  (reviewing.approval, review)
}

impl Worker {
  /// Starts auto mode's review of `approval`, which waits for the person, when its run uses auto mode.
  pub(in super::super) fn start_review(
    &mut self,
    watch: &mut Watch,
    approval: &PendingToolApproval,
  ) -> Result<Option<Reviewing>, StoreError> {
    if !watch.db.run_auto_mode(approval.run)? {
      return Ok(None);
    }
    // The agent's last messages may still wait to be written.
    self.flush(watch)?;
    let subject = ApprovalSubject::of(approval, &self.scope);
    let permissions = watch
      .invocation
      .permissions
      .as_ref()
      .map(|launch| launch.permissions().clone());
    let input = self.review_input(watch, approval, permissions.as_ref())?;
    let reviewer = self.reviewer(watch)?;
    let work = tokio::task::spawn_blocking(move || {
      let reviewer = reviewer.as_ref().map(|reviewer| reviewer as &dyn RequestReviewer);
      review_request(reviewer, (&subject, permissions.as_ref()), &input)
    });
    Ok(Some(Reviewing {
      approval: approval.id.clone(),
      work,
    }))
  }

  /// Records what auto mode did with request `id`, and answers it when the reviewer approved, unless the
  /// person answered first.
  pub(in super::super) async fn apply_review(
    &mut self,
    watch: &mut Watch,
    (id, review): (String, Review),
  ) -> Result<(), StoreError> {
    let waiting = watch.approval.as_ref().is_some_and(|(approval, _)| approval.id == id);
    if !waiting || watch.stop.is_some() {
      return Ok(());
    }
    let run = watch.invocation.run;
    watch.db.record_review(run, &id, &review)?;
    if !review.approves() {
      // The person decides, and now reads why auto mode left it to them.
      self.notifier.send(Event::ToolApprovalChanged { run, task: self.task });
      return Ok(());
    }
    let (approval, awaiting) = watch.approval.take().expect("the waiting request was matched above");
    let subject = ApprovalSubject::of(&approval, &self.scope);
    let settled = Settled::reviewed(&subject, review);
    if let Err(rejection) = self.settle(watch, &approval, awaiting, settled).await? {
      tracing::debug!(?rejection, "auto mode's approval did not reach the agent");
    }
    Ok(())
  }

  /// What the reviewer reads about `approval`: the request as recorded (already redacted), the step,
  /// the task and the agent's last messages, redacted as the request is.
  fn review_input(
    &self,
    watch: &Watch,
    approval: &PendingToolApproval,
    permissions: Option<&AgentPermissions>,
  ) -> Result<ReviewInput, StoreError> {
    let run = watch.db.run(approval.run).map_err(|error| StoreError::Corrupt {
      what: "run",
      detail: error.to_string(),
    })?;
    let step_name = run
      .as_ref()
      .and_then(|run| {
        let key = run.execution(approval.execution)?.step().clone();
        run
          .snapshot()
          .body()
          .step(&key)
          .map(|step| step.name.as_str().to_owned())
      })
      .unwrap_or_else(|| self.agent.clone());
    let task = watch.db.task(self.task)?;
    let messages: Vec<String> = watch
      .db
      .recent_agent_messages(approval.run, approval.execution, MESSAGES_READ)?
      .iter()
      .map(|message| self.redact(message))
      .collect();
    let writable_folders = permissions
      .and_then(AgentPermissions::granted_folders)
      .map(|folders| {
        folders
          .entries()
          .iter()
          .map(|folder| folder.lexical_path(&self.root).display().to_string())
          .collect()
      })
      .unwrap_or_default();
    let defaults = AgentPermissions::before_permissions();
    let step = ReviewedStep {
      name: step_name,
      repository: permissions.unwrap_or(&defaults).repository(),
      network: permissions.unwrap_or(&defaults).network(),
      writable_folders,
    };
    let task = ReviewedTask {
      title: task
        .as_ref()
        .map_or_else(String::new, |task| task.details().title.as_str().to_owned()),
      brief: task
        .as_ref()
        .map_or_else(String::new, |task| task.details().brief.clone()),
      criteria: task
        .as_ref()
        .map(|task| task.details().acceptance_criteria.clone())
        .unwrap_or_default(),
    };
    let request = ReviewedRequest {
      command: approval.command,
      server: approval.server.clone(),
      action: approval.action.clone(),
      arguments: (!approval.command).then(|| approval.scope.clone()).flatten(),
      note: approval.command.then(|| approval.scope.clone()).flatten(),
      working_folder: approval.target.clone(),
    };
    Ok(ReviewInput::new(request, step, task, &messages))
  }

  /// The project's reviewer: its Claude Code connection with auto mode's model, run in the repository
  /// with the agent environment; `None` when the project has no Claude Code connection.
  fn reviewer(&self, watch: &Watch) -> Result<Option<ClaudeReviewer<ProcessRunner>>, StoreError> {
    let connections = watch.db.connections()?;
    let Some(executable) = reviewer_connection(&connections).and_then(|connection| connection.executable()) else {
      return Ok(None);
    };
    let model = watch.db.auto_mode_settings()?.model;
    let mut environment = agent_environment(std::env::vars_os());
    watch.db.project_environment()?.add_to(&mut environment);
    Ok(Some(ClaudeReviewer::new(
      executable,
      model,
      (&self.root, environment),
      ProcessRunner::new(REVIEW_TIMEOUT),
    )))
  }
}

impl Settled {
  /// The reviewer's approval of a request about `subject`, as it is recorded and marked.
  fn reviewed(subject: &ApprovalSubject, review: Review) -> Self {
    let category = match subject {
      ApprovalSubject::Command(command) => Some(command.category()),
      ApprovalSubject::McpTool { .. } => None,
    };
    Self {
      review: Some(review),
      ..Self::by(DecidedBy::Reviewer, category)
    }
  }
}
