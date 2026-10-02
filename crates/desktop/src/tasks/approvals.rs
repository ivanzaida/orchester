//! Tool requests awaiting a person's decision, with the CLI of the agent that asks and whether its
//! step only reads: the approval dialog says what an approval means for that CLI (an approved
//! Codex command runs outside its sandbox, Claude Code has none) and that it can change files in a
//! step that only reads. Each comes with its run, task and step, and what its wider answers cover
//! ([`ApprovalChoices`]).

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{
  connection::{AgentConnection, AgentProvider, EngineVendor},
  id::TaskId,
  permission::{LaunchPermissions, RepositoryAccess},
  resource::{AgentProfile, ModelId},
  run::{PendingToolApproval, PinnedVersion, Run, RunNumber, StepExecutor},
  store::{ActivityStore, ConnectionStore, ResourceStore, RunStore, TaskStore},
  task::Task,
};
use orchester_store::{ProjectDb, StoreError};

pub use super::approval_choices::{AlwaysBlocked, AlwaysShown, ApprovalChoices, CommandAlways, SimilarShown};

/// A pending request and the CLI of its agent; `None` when that cannot be read any more.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShownApproval {
  pub approval: PendingToolApproval,
  pub provider: Option<AgentProvider>,
  /// Who receives the agent's prompts, as its engine is named.
  pub vendor: Option<EngineVendor>,
  /// The model the asking step runs on, by the run's pinned snapshot.
  pub model: Option<ModelId>,
  /// The asking invocation launched read-only, by its recorded launch permissions.
  pub read_only: bool,
  /// The run's number, its task's key and the step's name, while they can be read.
  pub run: Option<RunNumber>,
  pub task: Option<Arc<str>>,
  pub task_id: Option<TaskId>,
  pub step: Option<Arc<str>>,
  pub choices: ApprovalChoices,
}

/// The pending requests of the project at `repo_root`, oldest first, each with its agent's CLI.
pub fn pending(db: &ProjectDb, repo_root: &Path) -> Result<Vec<ShownApproval>, StoreError> {
  Ok(
    db.pending_tool_approvals_all()?
      .into_iter()
      .map(|approval| shown(db, repo_root, approval))
      .collect(),
  )
}

fn shown(db: &ProjectDb, repo_root: &Path, approval: PendingToolApproval) -> ShownApproval {
  // `.ok()`: an unreadable run only loses the names and the wider answers' details; the request itself
  // stays shown and decidable, and the list reports its own read errors.
  let run = db.run(approval.run).ok().flatten();
  let step = run.as_ref().and_then(|run| step_of(run, &approval));
  let (connection, model) = step.as_ref().map_or((None, None), |step| {
    (connection(db, step.profile), Some(step.model.clone()))
  });
  let launch = launched(db, &approval);
  let read_only = launch
    .as_ref()
    .is_some_and(|launch| launch.permissions().repository() == RepositoryAccess::ReadOnly);
  let provider = connection.as_ref().map(AgentConnection::provider);
  // The folders the agent may write to besides its worktree, as the runner classifies its commands.
  let writable: Vec<PathBuf> = launch
    .as_ref()
    .and_then(|launch| launch.permissions().granted_folders())
    .map(|folders| {
      folders
        .entries()
        .iter()
        .map(|folder| folder.lexical_path(repo_root))
        .collect()
    })
    .unwrap_or_default();
  let choices = ApprovalChoices::read(
    db,
    &approval,
    (
      step.as_ref().map(|step| step.profile.resource),
      step.as_ref().map(|step| step.profile.number),
    ),
    (&writable, provider == Some(AgentProvider::CodexCli)),
  );
  let task = run.as_ref().and_then(|run| db.task(run.task()).ok().flatten());
  ShownApproval {
    provider,
    vendor: connection.as_ref().map(AgentConnection::vendor),
    model,
    read_only,
    run: run.as_ref().map(Run::number),
    task_id: task.as_ref().map(Task::id),
    task: task.map(|task| Arc::from(task.key().to_string())),
    step: step.map(|step| step.name),
    choices,
    approval,
  }
}

/// The asking step as the run pinned it.
struct PinnedStep {
  name: Arc<str>,
  profile: PinnedVersion,
  model: ModelId,
}

fn step_of(run: &Run, approval: &PendingToolApproval) -> Option<PinnedStep> {
  let key = run.execution(approval.execution)?.step().clone();
  let pinned = run.snapshot().step(&key)?;
  let StepExecutor::Agent { profile, model, .. } = pinned.executor() else {
    return None;
  };
  let name = run.snapshot().body().step(&key)?.name.as_str().into();
  Some(PinnedStep {
    name,
    profile: *profile,
    model: model.clone(),
  })
}

/// The launch permissions the asking invocation recorded; `None` when unreadable or unrecorded, which
/// only loses the read-only wording.
fn launched(db: &ProjectDb, approval: &PendingToolApproval) -> Option<LaunchPermissions> {
  // `.ok()`: as for the provider, the request stays shown and decidable without the wording.
  db.invocations(approval.run)
    .ok()?
    .into_iter()
    .find(|invocation| invocation.id == approval.invocation)
    .and_then(|invocation| invocation.permissions)
}

/// The agent connection of pinned profile version `profile`, while both can be read.
fn connection(db: &ProjectDb, profile: PinnedVersion) -> Option<AgentConnection> {
  // `.ok()`: an unreadable profile or connection only loses the provider-specific wording;
  // a connection of another kind is no agent's.
  let version = db.version::<AgentProfile>(profile.resource, profile.number).ok()??;
  let connection = db.connection(version.body().connection()).ok()??;
  AgentConnection::try_from(connection).ok()
}
