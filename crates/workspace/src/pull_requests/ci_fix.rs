//! Fix CI on a run's pull request (`docs/design/pr-ci-status.md`, ORC-335): the failing checks the last read gave
//! ([`super::ci`]), with their pages and log tails ([`orchester_core::pull_request::ci::fix_text`], secrets redacted),
//! sent to the run's agent as one message, which goes as the composer's own would in the run's state
//! ([`crate::tasks::conversation::outgoing`]): delivered at the agent's next turn, following up a run ready for review,
//! running the task again after it ended. Fix CI waits while nothing failed, while an earlier Fix CI waits for its
//! agent, and while a message would decide for the person (a question, a plan) or cannot go (a limit, a closed run).
//! *Fix CI automatically* sends a failure no Fix CI answered yet on its own, up to the run's limit, then stops and
//! records that it did. Each send is recorded with the run (`orchester_core::pull_request::ci::fix`).

use std::path::Path;

use orchester_core::{
  conversation::Recipient,
  id::{IdempotencyKey, MessageId, RunId, TaskId},
  pull_request::ci::{
    fix::{AutoFix, AutoStep, FixEvent, FixEventKind, FixedBy, failure_of, is_failing},
    fix_text,
  },
  run::{Answerer, StepExecutor},
  store::RunStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::{
  InboxError,
  ci::{PullRequestCi, ReadCi, run_pull_request},
};
use crate::{
  background,
  messages::{self, Message},
  runner::Runners,
  sharing::Servers,
  tasks::{
    conversation::{
      Conversation,
      composer::{ComposerMode, Queued, Then},
      follow::{RunnerOf, Sent, Stored, follow},
      outgoing::{OutgoingError, Prepared, Way, conversation_of, offer as mode_of, prepare_in, store_answer_in},
    },
    detail::{RunView, TaskDetail, run_store_error},
  },
  workspace::OpenProject,
};

/// Whether Fix CI can send now, or why not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FixOffer {
  Available,
  Unavailable(Message),
}

/// Who sends a Fix CI: a press of *Fix CI* (`by` a person, or automation driving the window), or the run's *Fix CI
/// automatically*.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fixer {
  Hand { by: Answerer },
  Automatic,
}

/// Why a Fix CI was not stored.
#[derive(Debug)]
pub enum FixError {
  /// Fix CI cannot send now, for this reason.
  Unavailable(Message),
  Outgoing(OutgoingError),
  Store(StoreError),
}

impl FixError {
  pub fn message(&self) -> Message {
    match self {
      Self::Unavailable(reason) => reason.clone(),
      Self::Outgoing(error) => error.message(),
      Self::Store(error) => messages::store_error(error),
    }
  }
}

impl From<StoreError> for FixError {
  fn from(error: StoreError) -> Self {
    Self::Store(error)
  }
}

/// A Fix CI stored as the run's message, for its runner to be told.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredFix {
  pub task: TaskId,
  pub run: RunId,
  pub message: MessageId,
  pub by: FixedBy,
  key: IdempotencyKey,
  then: Then,
  pipeline: String,
}

/// What one Fix CI did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FixDone {
  Stored(StoredFix),
  /// *Fix CI automatically* reached its limit on this failure and recorded that it stopped.
  Stopped,
  /// *Fix CI automatically* had nothing to send.
  Nothing,
}

/// What Fix CI offers for `detail`'s latest run with `ci`, the run's pull request's checks; `lost`: the run is recorded
/// as running while no runner looks after it.
pub fn offer(detail: &TaskDetail, lost: bool, ci: &PullRequestCi) -> FixOffer {
  let unavailable = |key: &'static str| FixOffer::Unavailable(Message::new(key));
  let PullRequestCi::Read(read) = ci else {
    return unavailable("fixCi.unavailable.notRead");
  };
  if failure_of(&read.checks).is_none() {
    return unavailable(if read.checks.is_pending() {
      "fixCi.unavailable.running"
    } else {
      "fixCi.unavailable.nothingFailed"
    });
  }
  let Some(run) = &detail.run else {
    return unavailable("fixCi.unavailable.closed");
  };
  if waiting(detail, run) {
    return unavailable("fixCi.unavailable.waiting");
  }
  let (mode, _) = fix_mode(&conversation_of(detail, lost), run);
  match mode {
    ComposerMode::Answer { .. }
    | ComposerMode::PlanChanges { .. }
    | ComposerMode::Queue(Queued::Decision | Queued::AnswerAbove(_)) => unavailable("fixCi.unavailable.decision"),
    ComposerMode::RaiseLimit | ComposerMode::Queue(Queued::SharedBudget(_)) => unavailable("fixCi.unavailable.limit"),
    ComposerMode::Closed => unavailable("fixCi.unavailable.closed"),
    _ => FixOffer::Available,
  }
}

/// The step that wrote the code: the run's last step in pipeline order whose agent may write in its worktree. A Fix
/// CI goes to its agent, so a run that ended continues there (`docs/design/pr-ci-status.md`, "Fix CI").
pub fn writing_step(run: &RunView) -> Option<&crate::tasks::detail::StepView> {
  run.steps.iter().rev().find(|step| !step.tool_step && step.writes())
}

/// The agent a Fix CI goes to and what sending to it does in `conversation`: the writing step's agent
/// ([`writing_step`]), else the one the run's state addresses.
fn fix_mode(conversation: &Conversation, run: &RunView) -> (ComposerMode, Recipient) {
  let writer = writing_step(run)
    .and_then(|step| run.run.snapshot().step(&step.key))
    .and_then(|step| match step.executor() {
      StepExecutor::Agent { profile, .. } => Some(Recipient::Agent(profile.resource)),
      StepExecutor::Tool { .. } | StepExecutor::Test => None,
    });
  match writer {
    Some(recipient) => (ComposerMode::of(conversation, Some(run), recipient), recipient),
    None => mode_of(conversation, Some(run), None),
  }
}

/// Whether the run's last Fix CI still waits for its agent to take it.
fn waiting(detail: &TaskDetail, run: &RunView) -> bool {
  let Some(FixEventKind::Sent { message, .. }) = run.ci_fixes.last_sent().map(|event| event.kind) else {
    return false;
  };
  detail
    .messages
    .iter()
    .find(|stored| stored.id() == message)
    .and_then(orchester_core::conversation::Message::delivery)
    .is_some_and(|delivery| delivery == orchester_core::conversation::Delivery::Queued)
}

/// How a Fix CI goes in `mode`: after the turn of an agent at work, else as the composer's own first way.
fn way_in(mode: &ComposerMode) -> Way {
  match mode {
    ComposerMode::Steer(working) if working.mid_turn => Way::Second,
    _ => Way::Primary,
  }
}

/// Sets `run`'s *Fix CI automatically* as the person set it `by`, which restarts its count. Only a person sets it:
/// automation driving the window is refused, as a change by it would restart the count and so lift the run's limit.
/// Blocks on SQLite.
pub fn set_auto_fix_in(root: &Path, run: RunId, auto: AutoFix, by: Answerer) -> Result<(), FixError> {
  if by == Answerer::Automation {
    return Err(FixError::Unavailable(Message::new(
      "appMcp.errors.automationNotAllowed",
    )));
  }
  Ok(ProjectDb::open(root)?.set_auto_fix(run, auto, Timestamp::now())?)
}

/// The run `prepared` is for: the task's latest.
fn latest_run(prepared: &Prepared) -> &RunView {
  prepared
    .detail
    .run
    .as_ref()
    .expect("prepared for the task's latest run")
}

/// The failing checks' names of `read`, in `gh`'s order.
fn failing_names(read: &ReadCi) -> Vec<String> {
  read
    .checks
    .checks
    .iter()
    .filter(|check| is_failing(check.state))
    .map(|check| check.name.clone())
    .collect()
}

/// Stores the Fix CI of `read`, the checks of `task`'s latest run `run`'s pull request, sent by `fixer`, and records
/// it; or, for *Fix CI automatically*, records that it stopped at its limit, or does nothing. `lost` as in [`offer`].
/// Blocks on SQLite.
pub fn store_fix_in(
  root: &Path,
  (task, run): (TaskId, RunId),
  (read, lost): (&ReadCi, bool),
  fixer: Fixer,
) -> Result<FixDone, FixError> {
  let mut prepared = prepare_in(root, (task, Some(run)), (lost, None)).map_err(FixError::Outgoing)?;
  (prepared.mode, prepared.recipient) = fix_mode(&prepared.conversation, latest_run(&prepared));
  let view = latest_run(&prepared);
  let ci = PullRequestCi::Read(read.clone());
  if let FixOffer::Unavailable(reason) = offer(&prepared.detail, lost, &ci) {
    return match fixer {
      Fixer::Hand { .. } => Err(FixError::Unavailable(reason)),
      Fixer::Automatic => Ok(FixDone::Nothing),
    };
  }
  let failure = failure_of(&read.checks).expect("Fix CI is offered for failing checks only");
  let failing = failing_names(read);
  let way = way_in(&prepared.mode);
  let mut db = ProjectDb::open(root)?;
  // Automation never decides for the person: a run that would take the message as a decision waits for them.
  let decides = prepared.decision_kind(way).is_some();
  let (by, answerer) = match fixer {
    Fixer::Hand {
      by: Answerer::Automation,
    } if decides => {
      return Err(FixError::Unavailable(Message::new(
        "appMcp.errors.automationNotAllowed",
      )));
    }
    Fixer::Hand { by } => (FixedBy::Hand, by),
    Fixer::Automatic if decides => return Ok(FixDone::Nothing),
    Fixer::Automatic => match view.ci_fixes.auto_step(&failure) {
      AutoStep::Send { count, limit } => (FixedBy::Automatically { count, limit }, Answerer::Automation),
      AutoStep::Nothing => return Ok(FixDone::Nothing),
      AutoStep::Stop { limit } => {
        let event = FixEvent {
          at: Timestamp::now(),
          number: read.number,
          failure,
          failing,
          kind: FixEventKind::Stopped { limit },
        };
        db.record_ci_fix(run, &event)?;
        return Ok(FixDone::Stopped);
      }
    },
  };
  let text = fix_text(&read.checks, (read.number, view.run.branch().as_str()), by)
    .expect("Fix CI is offered for failing checks only");
  let key = IdempotencyKey::generate();
  let (then, message) =
    store_answer_in(root, &prepared, (key, way), (&text, &[], None), answerer).map_err(FixError::Outgoing)?;
  let event = FixEvent {
    at: message.created_at(),
    number: read.number,
    failure,
    failing,
    kind: FixEventKind::Sent {
      message: message.id(),
      by,
    },
  };
  db.record_ci_fix(run, &event)?;
  Ok(FixDone::Stored(StoredFix {
    task,
    run,
    message: message.id(),
    by,
    key,
    then,
    pipeline: view.pipeline.to_string(),
  }))
}

/// [`store_fix_in`] off the async runtime, then what follows the stored message through `open`'s runner, which starts
/// if the project has none: the message goes on as the composer's would, or stays queued with why.
pub async fn fix_ci(
  (runners, servers): (&Runners, &Servers),
  open: &OpenProject,
  (task, run): (TaskId, RunId),
  (read, lost): (ReadCi, bool),
  fixer: Fixer,
) -> Result<(FixDone, Option<Sent>), Message> {
  let root = open.repo_root.clone();
  let done = background::run(move || store_fix_in(&root, (task, run), (&read, lost), fixer))
    .await?
    .map_err(|error| {
      tracing::info!(?error, "Fix CI did not send");
      error.message()
    })?;
  let FixDone::Stored(fix) = &done else {
    return Ok((done, None));
  };
  let stored = Stored {
    root: &open.repo_root,
    task: fix.task,
    run: fix.run,
    key: fix.key,
    pipeline: &fix.pipeline,
  };
  let sent = follow((runners, servers, RunnerOf::Project(open)), &stored, fix.then).await;
  Ok((done, Some(sent)))
}

/// A run whose *Fix CI automatically* is on, with its pull request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AutoFixRun {
  pub task: TaskId,
  pub run: RunId,
  pub number: u64,
}

/// The runs of the project at `root` whose *Fix CI automatically* is on and whose branch has a pull request the last
/// sync stored. Blocks on SQLite.
pub fn auto_fix_runs_in(root: &Path) -> Result<Vec<AutoFixRun>, InboxError> {
  let db = ProjectDb::open(root)?;
  let mut runs = Vec::new();
  for id in db.runs_fixing_ci_automatically()? {
    let run = db
      .run(id)
      .map_err(run_store_error)?
      .ok_or(StoreError::RunNotFound(id))?;
    if let Some(number) = run_pull_request(&db, &run)? {
      runs.push(AutoFixRun {
        task: run.task(),
        run: id,
        number,
      });
    }
  }
  Ok(runs)
}
