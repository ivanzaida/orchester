//! Create & run on the new task form (02.09 `l2YA4`): check that the run can start, save the
//! task and queue it, then ask the runner to start its run. Every step is safe to retry: the
//! new draft is saved under the form's batch key or an existing draft is edited in place;
//! queueing a queued task changes nothing, and the run is started under the form's run key. A
//! shared task is claimed for the person before it is queued: someone else's task is not changed.

use std::{collections::BTreeMap, path::Path};

use orchester_core::{
  connection::AccountStartProblem,
  id::ResourceId,
  resource::{AgentProfile, Pipeline, ResourceBody, ResourceKind, Tool, ToolKind, Version, VersionNumber},
  run::{AutoAsk, PreflightInput, PreflightProblem, RunLimit, preflight},
  store::{ConnectionStore, ResourceStore, TaskStore},
  task::{QueueError, Task, TaskDetails, TaskStatus, TaskUpdate},
  time::Timestamp,
};
use orchester_ipc::Rejection;
use orchester_store::{ProjectDb, StoreError};

use super::{
  data::{PipelineChoice, SaveTarget, pinned_pipeline, save_to},
  writer::{TaskWriter, WriteError},
};
use crate::{
  messages::{self, Message},
  runner::{RequestError, RunRequestError},
  sharing::{
    Servers,
    project::{WorkClaim, WorkRefusal},
  },
};

/// Why the task was not queued for a run.
#[derive(Debug)]
pub enum QueueFailure {
  Store(StoreError),
  /// The shared project's server did not take the change.
  Write(Box<WriteError>),
  /// The shared task's claim was not taken: someone else holds it, or the server cannot be asked.
  Claim(Box<WorkRefusal>),
  /// The run could not start with the task's configuration; nothing was saved.
  Preflight(Vec<PreflightProblem>),
  Queue(QueueError),
  /// The task was changed since the form read it: nothing was saved or queued.
  Changed,
}

impl From<StoreError> for QueueFailure {
  fn from(error: StoreError) -> Self {
    Self::Store(error)
  }
}

impl From<WriteError> for QueueFailure {
  fn from(error: WriteError) -> Self {
    match error {
      WriteError::Store(error) => Self::Store(error),
      error => Self::Write(Box::new(error)),
    }
  }
}

impl QueueFailure {
  pub fn messages(&self) -> Vec<Message> {
    match self {
      Self::Store(error) => vec![messages::store_error(error)],
      Self::Write(error) => vec![error.message()],
      Self::Claim(refusal) => vec![refusal.message()],
      Self::Preflight(problems) => problems.iter().map(preflight_message).collect(),
      Self::Queue(_) => vec![Message::new("runs.start.incomplete")],
      Self::Changed => vec![messages::task_changed_meanwhile()],
    }
  }
}

/// The saved task and the exact pipeline version its queued run will use.
pub struct QueuedStart {
  pub task: Task,
  pub pipeline: PipelineChoice,
  /// The run claim queuing took or confirmed; a start that fails releases it if queuing took it.
  pub claim: WorkClaim,
}

pub fn queue_organized_in(
  repo_root: &Path,
  servers: &Servers,
  target: SaveTarget,
  details: TaskDetails,
  organization: (
    &orchester_core::task::organization::TaskOrganization,
    &orchester_core::task::organization::TaskOrganization,
  ),
  version: VersionNumber,
) -> Result<QueuedStart, QueueFailure> {
  let mut writer = TaskWriter::open(repo_root, servers)?;
  let existing = match target {
    SaveTarget::Task(task, _) => writer.db().task(task)?,
    SaveTarget::New(batch) => writer.db().task_by_batch(batch)?,
  };
  if let Some(task) = existing.filter(|task| task.status() == TaskStatus::Queued) {
    writer.db().check_task_readiness(task.id())?;
    return queued_start(writer.db(), task);
  }
  let problems = check(writer.db(), &details, version)?;
  if !problems.is_empty() {
    return Err(QueueFailure::Preflight(problems));
  }
  let task = saved(writer.save_organized(target, details.clone(), organization, Timestamp::now())?)?;
  writer.db().check_task_readiness(task.id())?;
  drop(writer);
  queue_in(
    repo_root,
    servers,
    SaveTarget::Task(task.id(), task.revision()),
    details,
    version,
  )
}

/// Checks the selected configuration, then saves and queues the task. A queued task reached by
/// an idempotent retry keeps its original version, even if a newer version was published. A run
/// claim taken for a shared task is released when the task is not queued after all.
pub fn queue_in(
  repo_root: &Path,
  servers: &Servers,
  target: SaveTarget,
  details: TaskDetails,
  selected_version: VersionNumber,
) -> Result<QueuedStart, QueueFailure> {
  let mut claimed = WorkClaim::default();
  match queue_claimed((repo_root, servers, &mut claimed), target, details, selected_version) {
    Ok(queued) => Ok(QueuedStart {
      claim: claimed,
      ..queued
    }),
    Err(failure) => {
      if let Ok(runtime) = tokio::runtime::Handle::try_current() {
        runtime.block_on(claimed.release(servers));
      }
      Err(failure)
    }
  }
}

fn queue_claimed(
  (repo_root, servers, claimed): (&Path, &Servers, &mut WorkClaim),
  target: SaveTarget,
  details: TaskDetails,
  selected_version: VersionNumber,
) -> Result<QueuedStart, QueueFailure> {
  let mut writer = TaskWriter::open(repo_root, servers)?;
  let existing = match target {
    SaveTarget::New(batch) => writer.db().task_by_batch(batch)?,
    SaveTarget::Task(id, _) => writer.db().task(id)?,
  };
  if let Some(task) = &existing {
    *claimed = std::mem::take(claimed).and(claim(&writer, servers, task.id())?);
  }
  if let Some(task) = existing.filter(|task| task.status() == TaskStatus::Queued) {
    writer.db().check_task_readiness(task.id())?;
    return queued_start(writer.db(), task);
  }
  let problems = check(writer.db(), &details, selected_version)?;
  if !problems.is_empty() {
    return Err(QueueFailure::Preflight(problems));
  }
  // No fallible pipeline read may follow a successful transition to Queued: the form must
  // receive that state even if the database becomes unreadable immediately afterwards.
  let selected_pipeline = load_pipeline(writer.db(), details.pipeline, selected_version)?;
  let mut task = saved(save_to(&mut writer, target, details, Timestamp::now())?)?;
  writer.db().check_task_readiness(task.id())?;
  if task.status() == TaskStatus::Draft {
    *claimed = std::mem::take(claimed).and(claim(&writer, servers, task.id())?);
    task.queue(selected_version).map_err(QueueFailure::Queue)?;
    let task = saved(writer.save_task(&task)?)?;
    return Ok(QueuedStart {
      task,
      pipeline: selected_pipeline,
      claim: WorkClaim::default(),
    });
  }
  // An idempotent save may return a task queued by another attempt; reload its persisted pin.
  queued_start(writer.db(), task)
}

/// Takes the run claim of a shared `task` before anything changes it for a run; a local task needs
/// none. Waits on the worker it runs on.
fn claim(writer: &TaskWriter, servers: &Servers, task: orchester_core::id::TaskId) -> Result<WorkClaim, QueueFailure> {
  let TaskWriter::Shared(store) = writer else {
    return Ok(WorkClaim::default());
  };
  let repo_root = store.server().repo_root();
  let runtime = tokio::runtime::Handle::try_current().map_err(|_| {
    QueueFailure::Claim(Box::new(WorkRefusal::Sharing {
      source: crate::sharing::SharingError::Interrupted,
      server: None,
    }))
  })?;
  runtime
    .block_on(servers.claim_for_work(repo_root, task))
    .map_err(|refusal| QueueFailure::Claim(Box::new(refusal)))
}

/// The saved task; a task changed since it was read is not saved, and nothing goes on.
fn saved(update: TaskUpdate) -> Result<Task, QueueFailure> {
  update.saved().ok_or(QueueFailure::Changed)
}

fn queued_start(db: &ProjectDb, task: Task) -> Result<QueuedStart, QueueFailure> {
  let number = task
    .pipeline_version()
    .ok_or_else(|| QueueFailure::Preflight(vec![PreflightProblem::PipelineVersionNotPinned]))?;
  let pipeline = load_pipeline(db, task.details().pipeline, number)?;
  Ok(QueuedStart {
    task,
    pipeline,
    claim: WorkClaim::default(),
  })
}

fn load_pipeline(
  db: &ProjectDb,
  pipeline_id: Option<ResourceId>,
  number: VersionNumber,
) -> Result<PipelineChoice, QueueFailure> {
  let id = pipeline_id.ok_or_else(|| QueueFailure::Preflight(vec![PreflightProblem::PipelineNotPublished]))?;
  pinned_pipeline(db, id, number)?.ok_or_else(|| QueueFailure::Preflight(vec![PreflightProblem::PipelineNotPublished]))
}

/// Problems the runner would refuse the run for, as far as the configuration shows them. The
/// runner checks again and also checks the repository.
pub(crate) fn check(
  db: &ProjectDb,
  details: &TaskDetails,
  selected_version: VersionNumber,
) -> Result<Vec<PreflightProblem>, StoreError> {
  let pipeline = match details.pipeline {
    Some(pipeline) => db.version::<Pipeline>(pipeline, selected_version)?,
    None => None,
  };
  let connections = db.connections()?;
  Ok(preflight(PreflightInput {
    pipeline: pipeline.as_ref().map(Version::body),
    profiles: &latest::<AgentProfile>(db, ResourceKind::AgentProfile)?,
    tools: &latest::<Tool>(db, ResourceKind::Tool)?,
    connections: &connections,
  }))
}

/// The latest published version of every resource of `kind`.
fn latest<B: ResourceBody>(db: &ProjectDb, kind: ResourceKind) -> Result<BTreeMap<ResourceId, Version<B>>, StoreError> {
  let mut versions = BTreeMap::new();
  for resource in db.resources(kind)? {
    if let Some(version) = db.latest_version::<B>(resource.id())? {
      versions.insert(resource.id(), version);
    }
  }
  Ok(versions)
}

/// What a person reads for a preflight problem.
pub fn preflight_message(problem: &PreflightProblem) -> Message {
  let step =
    |key: &'static str, step: &orchester_core::resource::StepKey| Message::new(key).with("step", step.to_string());
  match problem {
    PreflightProblem::PipelineNotPublished => Message::new("runs.preflight.pipelineNotPublished"),
    PreflightProblem::PipelineVersionNotPinned => Message::new("runs.preflight.pipelineVersionNotPinned"),
    PreflightProblem::NoSteps => Message::new("runs.preflight.noSteps"),
    PreflightProblem::ToolNotPublished { step: key } => step("runs.preflight.toolNotPublished", key),
    PreflightProblem::ToolNotRunnable {
      step: key,
      kind: ToolKind::Http,
    } => step("runs.preflight.toolNotRunnable.http", key),
    PreflightProblem::ToolNotRunnable {
      step: key,
      kind: ToolKind::Database,
    } => step("runs.preflight.toolNotRunnable.database", key),
    PreflightProblem::ToolNotRunnable {
      step: key,
      kind: ToolKind::Command,
    } => step("runs.preflight.toolNotRunnable.command", key),
    PreflightProblem::ToolAccessRules { step: key } => step("runs.preflight.toolAccessRules", key),
    PreflightProblem::ApprovalGate { step: key } => step("runs.preflight.approvalGate", key),
    PreflightProblem::ProfileNotPublished { step: key } => step("runs.preflight.profileNotPublished", key),
    PreflightProblem::ConnectionMissing { step: key } => step("runs.preflight.connectionMissing", key),
    PreflightProblem::UnsupportedProvider { step: key } => step("runs.preflight.unsupportedProvider", key),
    PreflightProblem::NativeNotAvailable { step: key } => step("runs.preflight.nativeNotAvailable", key),
    PreflightProblem::ConnectionNotUsable { step: key, .. } => step("runs.preflight.connectionNotUsable", key),
    PreflightProblem::AccountUnavailable { step: key, reason } => match reason {
      AccountStartProblem::NoAccount => step("runs.preflight.account.none", key),
      AccountStartProblem::FolderMissing { folder } => {
        step("runs.preflight.account.folderMissing", key).with("folder", folder.display().to_string())
      }
      AccountStartProblem::Unsupported => step("runs.preflight.account.unsupported", key),
    },
    PreflightProblem::PermissionsNotEnforceable { step: key, .. } => {
      step("runs.preflight.permissionsNotEnforceable", key)
    }
    PreflightProblem::CommandListNotOffered { step: key } => step("runs.preflight.commandListNotOffered", key),
    PreflightProblem::NotAGitRepository => Message::new("runs.preflight.notAGitRepository"),
    PreflightProblem::NoCommits => Message::new("runs.preflight.noCommits"),
  }
}

/// What a person reads when the start request fails, including an unconfirmed outcome.
pub fn start_messages(error: &RunRequestError) -> Vec<Message> {
  match error {
    RunRequestError::Rejected(Rejection::RunAgainBlocked {
      block: orchester_core::run::RunAgainBlock::TaskBlocked,
    }) => vec![Message::new("tasks.blocked.runUnavailable")],
    RunRequestError::Request(RequestError::NotConnected) => vec![Message::new("runs.start.runnerUnavailable")],
    RunRequestError::Request(RequestError::Lost) => vec![Message::new("runs.start.lost")],
    RunRequestError::Rejected(Rejection::Preflight { problems }) => problems.iter().map(preflight_message).collect(),
    RunRequestError::Rejected(Rejection::TaskNotFound) => vec![Message::new("runs.start.taskMissing")],
    RunRequestError::Rejected(Rejection::TaskNotQueued) => vec![Message::new("runs.start.notQueued")],
    RunRequestError::Rejected(Rejection::TaskChangedMeanwhile) => vec![crate::messages::task_changed_meanwhile()],
    RunRequestError::Rejected(Rejection::TaskGroupWaiting { hold }) => super::group::hold_messages(hold),
    RunRequestError::Rejected(Rejection::RunUnreadable { .. }) => vec![Message::new("runs.unreadable.rejected")],
    RunRequestError::Rejected(
      rejection @ (Rejection::DailyBudgetUsed { .. } | Rejection::NoAgentSlot { .. } | Rejection::ShuttingDown),
    ) => {
      vec![capacity_message(rejection)]
    }
    RunRequestError::Rejected(
      Rejection::Failed { .. }
      | Rejection::RunNotInterrupted
      | Rejection::RunNotWaiting
      | Rejection::RunNotFound
      | Rejection::RunNotRunning
      | Rejection::RunNotPaused
      | Rejection::NotAnotherAccount
      | Rejection::RunEnded
      | Rejection::WorktreeRemoved
      | Rejection::LimitNotRaised { .. }
      | Rejection::LimitNotAboveUsed { .. }
      | Rejection::ProfileNotInRun
      | Rejection::PermissionsNotEnforceable { .. }
      | Rejection::NoAgentToRestart
      | Rejection::NoPendingPermissions
      | Rejection::PlanNotAwaited
      | Rejection::ToolApprovalNotAwaited
      | Rejection::ToolApprovalAnswered
      | Rejection::AutomationWider
      | Rejection::PersonOnly { .. }
      | Rejection::ChatMessageNotFound
      | Rejection::ChatBusy
      | Rejection::NoOrchestrator
      | Rejection::QuestionNotAwaited
      | Rejection::QuestionAnswered
      | Rejection::InvalidAnswer { .. }
      | Rejection::RunAgainBlocked { .. }
      | Rejection::WorktreeChanged { .. }
      | Rejection::WorktreeNotRemoved { .. }
      | Rejection::Busy,
    ) => vec![Message::new("runs.start.failed")],
  }
}

/// Whether a failed start request definitely stopped before a run was created. Once a reply is
/// lost or the runner reports a failure, a later preflight rejection cannot prove that an
/// earlier request did not already create the run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartOutcome {
  ConfirmedNotStarted,
  Unconfirmed,
}

impl StartOutcome {
  pub fn after(self, error: &RunRequestError) -> Self {
    if self == Self::Unconfirmed
      || matches!(
        error,
        RunRequestError::Request(RequestError::Lost) | RunRequestError::Rejected(Rejection::Failed { .. })
      )
    {
      Self::Unconfirmed
    } else {
      Self::ConfirmedNotStarted
    }
  }

  pub fn status_key(self) -> &'static str {
    match self {
      Self::ConfirmedNotStarted => "tasks.new.queued",
      Self::Unconfirmed => "tasks.new.unconfirmed",
    }
  }

  pub fn description_key(self) -> &'static str {
    match self {
      Self::ConfirmedNotStarted => "tasks.new.queuedDescription",
      Self::Unconfirmed => "tasks.new.unconfirmedDescription",
    }
  }
}

/// What a person reads when the runner did not pause, resume or cancel a run, or raise its
/// limits.
pub fn control_message(error: &RunRequestError) -> Message {
  match error {
    RunRequestError::Request(RequestError::NotConnected) => Message::new("runs.control.runnerUnavailable"),
    RunRequestError::Request(RequestError::Lost) => Message::new("runs.control.lost"),
    RunRequestError::Rejected(Rejection::RunNotRunning) => Message::new("runs.control.notRunning"),
    RunRequestError::Rejected(Rejection::RunNotPaused) => Message::new("runs.control.notPaused"),
    RunRequestError::Rejected(Rejection::NotAnotherAccount) => Message::new("runs.control.notAnotherAccount"),
    RunRequestError::Rejected(Rejection::RunEnded) => Message::new("runs.control.ended"),
    RunRequestError::Rejected(Rejection::WorktreeRemoved) => Message::new("runs.control.worktreeRemoved"),
    RunRequestError::Rejected(Rejection::RunNotInterrupted) => Message::new("runs.control.notInterrupted"),
    RunRequestError::Rejected(Rejection::RunNotWaiting) => Message::new("runs.control.notWaiting"),
    RunRequestError::Rejected(Rejection::TaskGroupWaiting { hold }) => super::group::hold_messages(hold)
      .into_iter()
      .next()
      .unwrap_or_else(|| Message::new("runs.control.failed")),
    RunRequestError::Rejected(Rejection::RunUnreadable { .. }) => Message::new("runs.unreadable.rejected"),
    RunRequestError::Rejected(
      rejection @ (Rejection::DailyBudgetUsed { .. } | Rejection::NoAgentSlot { .. } | Rejection::ShuttingDown),
    ) => capacity_message(rejection),
    RunRequestError::Rejected(Rejection::LimitNotRaised { .. }) => Message::new("runs.limit.notRaised"),
    RunRequestError::Rejected(Rejection::LimitNotAboveUsed {
      limit: RunLimit::Tokens,
    }) => Message::new("runs.limit.tokensNotAboveUsed"),
    RunRequestError::Rejected(Rejection::LimitNotAboveUsed { limit: RunLimit::Time }) => {
      Message::new("runs.limit.timeNotAboveUsed")
    }
    RunRequestError::Rejected(Rejection::LimitNotAboveUsed {
      limit: RunLimit::StepBudget { .. },
    }) => Message::new("runs.limit.stepNotAboveUsed"),
    // An account's plan limit is not raised by Orchester; no runner refuses it for being too low.
    RunRequestError::Rejected(Rejection::LimitNotAboveUsed {
      limit: RunLimit::PlanLimit { .. },
    }) => Message::new("runs.control.failed"),
    RunRequestError::Rejected(Rejection::PermissionsNotEnforceable { step, .. }) => {
      Message::new("runs.permissions.errors.notEnforceable").with("step", step.to_string())
    }
    RunRequestError::Rejected(Rejection::NoAgentToRestart) => Message::new("runs.permissions.errors.noAgentToRestart"),
    RunRequestError::Rejected(Rejection::NoPendingPermissions) => Message::new("runs.permissions.errors.noPending"),
    RunRequestError::Rejected(Rejection::PlanNotAwaited) => Message::new("runs.plan.errors.notAwaited"),
    RunRequestError::Rejected(Rejection::ToolApprovalNotAwaited) => Message::new("runs.toolApproval.notAwaited"),
    RunRequestError::Rejected(Rejection::ToolApprovalAnswered) => Message::new("runs.toolApproval.answeredAlready"),
    RunRequestError::Rejected(Rejection::AutomationWider) => Message::new("runs.toolApproval.automationWider"),
    RunRequestError::Rejected(Rejection::PersonOnly { ask }) => Message::new(person_only_key(*ask)),
    RunRequestError::Rejected(Rejection::QuestionNotAwaited) => {
      Message::new("runs.conversation.question.errors.notAwaited")
    }
    RunRequestError::Rejected(Rejection::QuestionAnswered) => {
      Message::new("runs.conversation.question.errors.answered")
    }
    RunRequestError::Rejected(Rejection::InvalidAnswer { error }) => super::question::answer_error(error),
    // Unfinished blocking tasks keep the run from going on; the task page names them.
    RunRequestError::Rejected(Rejection::RunAgainBlocked {
      block: orchester_core::run::RunAgainBlock::TaskBlocked,
    }) => Message::new("tasks.blocked.runUnavailable"),
    RunRequestError::Rejected(
      Rejection::RunNotFound
      | Rejection::ProfileNotInRun
      | Rejection::LimitNotAboveUsed {
        limit: RunLimit::DailyBudget | RunLimit::GroupBudget,
      }
      | Rejection::ChatMessageNotFound
      | Rejection::ChatBusy
      | Rejection::NoOrchestrator
      | Rejection::TaskNotFound
      | Rejection::TaskNotQueued
      | Rejection::TaskChangedMeanwhile
      | Rejection::Preflight { .. }
      | Rejection::Failed { .. }
      | Rejection::RunAgainBlocked { .. }
      | Rejection::WorktreeChanged { .. }
      | Rejection::WorktreeNotRemoved { .. }
      | Rejection::Busy,
    ) => Message::new("runs.control.failed"),
  }
}

/// Why automation may not approve a request that auto-approval leaves to a person.
const fn person_only_key(ask: AutoAsk) -> &'static str {
  match ask {
    AutoAsk::Unknown => "runs.toolApproval.personOnly.unknown",
    AutoAsk::Destructive => "runs.toolApproval.personOnly.destructive",
    AutoAsk::OutsideWrite => "runs.toolApproval.personOnly.outsideWrite",
    AutoAsk::Secrets => "runs.toolApproval.personOnly.secrets",
    AutoAsk::Installs => "runs.toolApproval.personOnly.installs",
    AutoAsk::Network => "runs.toolApproval.personOnly.network",
  }
}

/// Why no run starts or continues now: today's budget is used, every agent slot is taken or the
/// runner is stopping.
fn capacity_message(rejection: &Rejection) -> Message {
  match rejection {
    Rejection::DailyBudgetUsed { .. } => Message::new("runs.capacity.dailyBudgetUsed"),
    Rejection::NoAgentSlot { slots } => Message::new("runs.capacity.noAgentSlot").with("slots", slots.to_string()),
    Rejection::ShuttingDown => Message::new("runs.capacity.shuttingDown"),
    other => {
      tracing::error!(?other, "not a capacity rejection");
      Message::new("runs.control.failed")
    }
  }
}
