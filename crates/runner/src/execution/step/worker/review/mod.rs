//! The reviewer in the worker (`orchester_core::run::review`, `docs/design/approval-modes.md`): a request the run's
//! mode asks the reviewer about waits under review, without asking the person (no dialog, no *Needs you*), while the
//! reviewer judges it on the blocking pool, so the worker keeps following its agent; it asks the person once the
//! reviewer asks, fails or times out, with why (`orchester_core::run::after_review`). Manual asks it whether to approve
//! what the profile does not allow, after its hard rules, which ask the person at once. Autopilot asks it one question
//! about a request Orchester could not place: whether it touches anything outside the run's roots. An approval answers
//! the request as the reviewer's. In a project without a reviewer to ask (no connection a reviewer runs on, or the
//! chosen one gone) such a request asks the person at once. A person who answers first wins, and the review is
//! stopped; a review whose run changed mode meanwhile is dropped, and the request is answered by the mode it is in
//! now. A Codex escalation (a command outside its sandbox) is reviewed under the same hard rules, which keep it from
//! the reviewer where the step's network is off or it only reads. What a review that asked a model cost is recorded
//! when it ends, dropped or not, and counts like a turn of the agent's ([`ledger`]).

mod estimate;
mod ledger;
mod native;
mod process;
mod setup;

use std::time::Duration;

use orchester_core::{
  permission::{AgentPermissions, category::build_variables, roots::RunRoots},
  run::{
    ApprovalSubject, CameToPerson, DecidedBy, PendingToolApproval, PersonReason, RequestAnswer, ReviewQuestion,
    after_review,
    review::{
      Review, ReviewFailure, ReviewHold, ReviewInput, ReviewedRequest, ReviewedStep, ReviewedTask, RootsQuestion,
      StepLimit, step_limit,
    },
    without_reviewer,
  },
  store::{ActivityStore, RunStore, SettingsStore, TaskStore},
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_store::StoreError;
use tokio::{runtime::Handle, task::JoinHandle};

pub(in crate::execution) use self::{estimate::estimate_task, setup::ReviewerKeys};
use self::{
  ledger::ReviewLedger,
  process::{StopReview, stop_review},
  setup::{PushPlace, ReviewJob, ReviewerSetup, hold_now, ignored_build_output},
};
use super::{super::answer::Settled, Watch, Worker};

/// The longest a review may take; the reviewer is stopped then, and the person decides.
const REVIEW_TIMEOUT: Duration = Duration::from_secs(60);
/// How many of the agent's last messages the reviewer reads.
const MESSAGES_READ: usize = 3;

/// A review under way: the request it is about, what the reviewer was asked, and the work that reviews it, stopped
/// when dropped.
pub(in super::super) struct Reviewing {
  approval: String,
  question: ReviewQuestion,
  work: JoinHandle<Review>,
  _stop: StopReview,
}

/// A review that ended: the request, what the reviewer was asked, and what it did.
pub(super) struct Reviewed {
  approval: String,
  question: ReviewQuestion,
  review: Review,
}

/// Ready once the review under way in `slot` ends.
pub(super) async fn next(slot: &mut Option<Reviewing>) -> Reviewed {
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
  Reviewed {
    approval: reviewing.approval,
    question: reviewing.question,
    review,
  }
}

impl Worker {
  /// Starts the reviewer's review of `approval` on `question`, which keeps it from the person until it ends; why the
  /// request comes to the person at once instead: no reviewer to ask, or one of Manual's hard rules. `escalation`:
  /// Codex asks to run it outside its sandbox.
  pub(in super::super) fn start_review(
    &mut self,
    watch: &mut Watch,
    approval: &PendingToolApproval,
    (question, escalation): (ReviewQuestion, bool),
  ) -> Result<Result<Reviewing, CameToPerson>, StoreError> {
    let setup = self.reviewer(watch)?;
    if matches!(setup, ReviewerSetup::Missing) {
      return Ok(Err(without_reviewer(question)));
    }
    let subject = ApprovalSubject::of(approval, &self.scope);
    let permissions = watch
      .invocation
      .permissions
      .as_ref()
      .map(|launch| launch.permissions().clone());
    let input = self.review_input(watch, approval, permissions.as_ref())?;
    let push = PushPlace {
      branch: watch
        .db
        .run(approval.run)
        .map_err(|error| StoreError::Corrupt {
          what: "run",
          detail: error.to_string(),
        })?
        .map(|run| run.branch().as_str().to_owned()),
      worktree: self.scope.worktree().to_path_buf(),
      origin: watch.db.run_origin(approval.run)?,
    };
    // A hard rule asks the person at once; anything else waits, without asking, for the reviewer.
    let ignored = ignored_build_output(self.scope.worktree(), &subject, permissions.as_ref());
    let (hold, roots) = match question {
      ReviewQuestion::Request => (
        hold_now((&subject, permissions.as_ref(), escalation), (&push, &ignored), &input),
        None,
      ),
      ReviewQuestion::Roots => (
        match step_limit(&subject, permissions.as_ref(), &ignored) {
          StepLimit::Held(hold) => Some(hold),
          StepLimit::Within | StepLimit::Unread => input.as_ref().err().copied(),
        },
        Some(roots_question(self.roots.as_ref(), push.branch.as_deref())),
      ),
    };
    if let Some(hold) = hold {
      watch
        .db
        .record_review(approval.run, &approval.id, &Review::Held { hold })?;
      return Ok(Err(CameToPerson::held(hold)));
    }
    // Marked last, after everything that can fail: a request marked under review always gets its review.
    watch.db.set_reviewing(approval.run, &approval.id, true)?;
    let (stop, stopped) = stop_review();
    let job = ReviewJob {
      subject,
      permissions,
      push,
      ignored,
      escalation,
      roots,
      input,
      runtime: Handle::current(),
      timeout: REVIEW_TIMEOUT,
      stop: stopped,
    };
    let ledger = ReviewLedger {
      root: self.root.clone(),
      run: approval.run,
      execution: approval.execution,
      invocation: approval.invocation,
      approval: approval.id.clone(),
      events: self.events.clone(),
      notifier: self.notifier.clone(),
      _held: self.reviews_held.clone(),
    };
    let work = tokio::task::spawn_blocking(move || setup.review(job, &ledger));
    Ok(Ok(Reviewing {
      approval: approval.id.clone(),
      question,
      work,
      _stop: stop,
    }))
  }

  /// Records what the reviewer did with its request, and answers it when the reviewer approved, unless the person
  /// answered first; else the request comes to the person with why. A review whose run changed mode meanwhile is
  /// dropped, and the request is answered by the mode it is in now.
  pub(super) async fn apply_review(&mut self, watch: &mut Watch, reviewed: Reviewed) -> Result<(), StoreError> {
    let Reviewed {
      approval: id,
      question,
      review,
    } = reviewed;
    let waiting = watch.approval.as_ref().is_some_and(|(approval, _)| approval.id == id);
    let run = watch.invocation.run;
    if !waiting || watch.stop.is_some() {
      return Ok(());
    }
    let autopilot = watch.in_autopilot()?;
    if autopilot != (question == ReviewQuestion::Roots) {
      watch.db.set_reviewing(run, &id, false)?;
      let waiting = watch.approval.take().expect("the waiting request was matched above");
      return self.route(watch, waiting, Timestamp::now()).await;
    }
    if let Err(error) = watch.db.record_review(run, &id, &review) {
      // The review is lost, but the request must not stay hidden: the person decides.
      watch.db.set_reviewing(run, &id, false)?;
      let unsure = CameToPerson::because(PersonReason::ReviewerUnsure);
      self.ask_person(watch, (&id, &unsure), autopilot)?;
      return Err(error);
    }
    let answer = match after_review(question, &review) {
      Ok(answer) => answer,
      // The person decides, and now reads why the reviewer left it to them.
      Err(came) => return self.ask_person(watch, (&id, &came), autopilot),
    };
    let (approval, awaiting) = watch.approval.take().expect("the waiting request was matched above");
    let subject = ApprovalSubject::of(&approval, &self.scope);
    let settled = Settled::reviewed(&subject, review, answer);
    if let Err(rejection) = self.settle(watch, &approval, awaiting, settled).await? {
      tracing::debug!(?rejection, "the reviewer's approval did not reach the agent");
    }
    Ok(())
  }

  /// The waiting request `id` comes to the person from now on, for `came`, under Autopilot or Manual (`autopilot`):
  /// the agent's time stops counting, and the app shows it.
  fn ask_person(
    &self,
    watch: &mut Watch,
    (id, came): (&str, &CameToPerson),
    autopilot: bool,
  ) -> Result<(), StoreError> {
    let came = came.clone().in_mode(autopilot);
    watch.db.record_came_to_person(watch.invocation.run, id, Some(&came))?;
    watch.asked_in_autopilot = autopilot;
    watch.invocation.timing.wait_for_person(Timestamp::now());
    // A person's wait never holds one of the computer's QA slots.
    self.qa.release(watch.invocation.id);
    watch.db.save_invocation(&watch.invocation)?;
    self.notifier.send(Event::ToolApprovalChanged {
      run: watch.invocation.run,
      task: self.task,
    });
    Ok(())
  }

  /// What the step's own permissions make of `subject` in Autopilot (`orchester_core::run::review::step_limit`),
  /// asking git which of its Cargo builds' target directories the worktree ignores where the step only reads.
  pub(in super::super) fn step_limit(&self, watch: &Watch, subject: &ApprovalSubject) -> StepLimit {
    let permissions = watch.invocation.permissions.as_ref().map(|launch| launch.permissions());
    let ignored = ignored_build_output(self.scope.worktree(), subject, permissions);
    step_limit(subject, permissions, &ignored)
  }

  /// What the reviewer reads about `approval`: the request as recorded (already redacted), the step,
  /// the task and the agent's last messages, redacted as the request is; a hold when the request is too
  /// long to be reviewed whole.
  fn review_input(
    &self,
    watch: &Watch,
    approval: &PendingToolApproval,
    permissions: Option<&AgentPermissions>,
  ) -> Result<Result<ReviewInput, ReviewHold>, StoreError> {
    let task = watch.db.task(self.task)?;
    let messages: Vec<String> = watch
      .db
      .recent_agent_messages(approval.run, approval.execution, MESSAGES_READ)?
      .iter()
      .map(|message| self.redact(message))
      .collect();
    let defaults = AgentPermissions::before_permissions();
    let step = ReviewedStep {
      name: self.step_name(watch, approval)?,
      repository: permissions.unwrap_or(&defaults).repository(),
      network: permissions.unwrap_or(&defaults).network().clone(),
      writable_folders: self.writable_folders(permissions),
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
    let variables = build_variables(&watch.db.project_environment()?);
    Ok(ReviewInput::new(request, step, task, &messages).map(|input| input.with_build_variables(variables)))
  }

  /// The name of the step that asks, from the run's pipeline; the agent's when it is not found.
  fn step_name(&self, watch: &Watch, approval: &PendingToolApproval) -> Result<String, StoreError> {
    let run = watch.db.run(approval.run).map_err(|error| StoreError::Corrupt {
      what: "run",
      detail: error.to_string(),
    })?;
    Ok(
      run
        .as_ref()
        .and_then(|run| {
          let key = run.execution(approval.execution)?.step().clone();
          run
            .snapshot()
            .body()
            .step(&key)
            .map(|step| step.name.as_str().to_owned())
        })
        .unwrap_or_else(|| self.agent.clone()),
    )
  }

  /// The folders outside the worktree the step may write to, as paths.
  fn writable_folders(&self, permissions: Option<&AgentPermissions>) -> Vec<String> {
    permissions
      .and_then(AgentPermissions::granted_folders)
      .map(|folders| {
        folders
          .entries()
          .iter()
          .map(|folder| folder.lexical_path(&self.root).display().to_string())
          .collect()
      })
      .unwrap_or_default()
  }
}

impl Settled {
  /// The reviewer's approval of a request about `subject`, as it is recorded and marked, with its reason.
  fn reviewed(subject: &ApprovalSubject, review: Review, answer: RequestAnswer) -> Self {
    let category = match subject {
      ApprovalSubject::Command(command) => Some(command.category()),
      ApprovalSubject::McpTool { .. } => None,
    };
    Self {
      review: Some(review),
      answer,
      ..Self::by(DecidedBy::Reviewer, category)
    }
  }
}

/// Autopilot's question about a request of a run on `branch` with `roots`; none when they could not be read, which
/// the reviewer then reads as nothing being inside.
fn roots_question(roots: Option<&RunRoots>, branch: Option<&str>) -> RootsQuestion {
  RootsQuestion {
    roots: roots
      .map(|roots| {
        roots
          .roots()
          .iter()
          .map(|root| root.path().display().to_string())
          .collect()
      })
      .unwrap_or_default(),
    branch: branch.unwrap_or_default().to_owned(),
  }
}
