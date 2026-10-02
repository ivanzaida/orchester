//! Tool requests awaiting a person's decision, with the CLI of the agent that asks and whether its
//! step only reads: the approval dialog says what an approval means for that CLI (an approved
//! Codex command runs outside its sandbox, Claude Code has none) and that it can change files in a
//! step that only reads. Each comes with its run, task and step, and what its wider answers cover
//! ([`ApprovalChoices`]). On its task's Conversation tab the request is a card of the timeline, and
//! the dialog does not open there.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{
  connection::{AgentConnection, AgentProvider, EngineVendor},
  id::TaskId,
  integration::WORKTREES_DIR,
  permission::{CommandPermission, LaunchPermissions, RepositoryAccess},
  resource::{AgentProfile, ModelId, VersionNumber},
  run::{PendingToolApproval, PinnedVersion, Run, RunNumber, StepExecutor, auto_approves},
  store::{ActivityStore, ConnectionStore, ResourceStore, RunStore, TaskStore},
  task::Task,
};
use orchester_store::{ProjectDb, StoreError};

pub use super::approval_choices::{AlwaysBlocked, AlwaysShown, ApprovalChoices, CommandAlways, SimilarShown};
use super::changes::run_worktree;
use crate::navigation::route::{Page, TaskPageTab};

/// What made a request ask, as the dialog names it beside its category (`Cv2MG` in `kpuQj`): the
/// asking profile's setting, at the version the run pinned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AskingPolicy {
  /// `None` when the invocation's permissions were not recorded.
  pub setting: Option<AskingSetting>,
  pub profile: Arc<str>,
  pub version: VersionNumber,
}

/// The profile setting a request asks under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AskingSetting {
  /// The profile asks before every command.
  CommandsAsk,
  /// The command is not one of the profile's Allowed commands.
  NotListed,
  /// Commands run in the CLI's sandbox, and this one asks to leave it (Codex).
  BeyondSandbox,
  /// The profile asks before MCP actions.
  McpActions,
}

impl AskingSetting {
  const fn of(commands: &CommandPermission, command: bool) -> Self {
    match (command, commands) {
      (false, _) => Self::McpActions,
      (true, CommandPermission::AskBefore) => Self::CommandsAsk,
      (true, CommandPermission::Allowed(_)) => Self::NotListed,
      (true, CommandPermission::AnyInWorktree) => Self::BeyondSandbox,
    }
  }
}

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
  /// A command's working folder relative to the repository when it lies in it, else as recorded.
  pub cwd: Option<String>,
  pub choices: ApprovalChoices,
  /// What made the request ask, while the asking profile version can be read.
  pub asking: Option<AskingPolicy>,
  /// The run already answers its requests itself; this one asks as auto-approval leaves it to a person.
  pub auto_approving: bool,
}

impl ShownApproval {
  /// Whether the dialog opens for it on `page`: everywhere but its task's Conversation tab, which
  /// shows it inline.
  pub fn opens_on(&self, page: Option<Page>) -> bool {
    !matches!((page, self.task_id), (Some(Page::Task(shown, TaskPageTab::Conversation)), Some(task)) if shown == task)
  }

  /// A command's first line, and whether more lines follow, for a one-line mention of it.
  pub fn first_line(&self) -> (&str, bool) {
    let mut lines = self.approval.action.lines();
    let first = lines.next().unwrap_or_default();
    (first, lines.next().is_some())
  }
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
  // `.ok()`: an unreadable profile version only loses the provider-specific wording and the policy.
  let version = step.as_ref().and_then(|step| {
    db.version::<AgentProfile>(step.profile.resource, step.profile.number)
      .ok()
      .flatten()
  });
  let connection = version.as_ref().and_then(|version| connection(db, version.body()));
  let model = step.as_ref().map(|step| step.model.clone());
  let launch = launched(db, &approval);
  let asking = version.as_ref().map(|version| AskingPolicy {
    setting: launch
      .as_ref()
      .map(|launch| AskingSetting::of(launch.permissions().commands(), approval.command)),
    profile: version.body().role().as_str().into(),
    version: version.number(),
  });
  // `.ok()`: an unreadable choice only shows Approve everything in this run as still to be chosen.
  let auto_approving = auto_approves(
    db.run_auto_approve(approval.run).ok().flatten(),
    launch
      .as_ref()
      .is_some_and(|launch| launch.permissions().auto_approve()),
  );
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
    (
      &agent_worktree(repo_root, &approval),
      &writable,
      provider == Some(AgentProvider::CodexCli),
    ),
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
    cwd: approval
      .command
      .then(|| relative_cwd(repo_root, approval.target.as_deref()))
      .flatten(),
    choices,
    asking,
    auto_approving,
    approval,
  }
}

/// The worktree the asking agent works in, as the runner made it: its step execution's own when the step
/// runs beside others of its run, else the run's.
fn agent_worktree(repo_root: &Path, approval: &PendingToolApproval) -> PathBuf {
  let step = repo_root.join(WORKTREES_DIR).join(approval.execution.to_string());
  if step.is_dir() {
    step
  } else {
    run_worktree(repo_root, approval.run)
  }
}

/// `target` as the repository names it, when it lies in `repo_root`.
fn relative_cwd(repo_root: &Path, target: Option<&str>) -> Option<String> {
  let target = Path::new(target?);
  Some(target.strip_prefix(repo_root).map_or_else(
    |_| target.display().to_string(),
    |relative| relative.display().to_string(),
  ))
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

/// The agent connection of `profile`, while it can be read.
fn connection(db: &ProjectDb, profile: &AgentProfile) -> Option<AgentConnection> {
  // `.ok()`: an unreadable connection only loses the provider-specific wording; a connection of
  // another kind is no agent's.
  let connection = db.connection(profile.connection()).ok()??;
  AgentConnection::try_from(connection).ok()
}
