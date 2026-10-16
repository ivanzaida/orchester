//! What the status bar shows, as plain data: the active tab's project with its runner, tasks and usage,
//! the session's activity, and the app's MCP server. The window gathers it ([`super::StatusBar`]); a test
//! hands it over directly. What the bar's items do goes back as a [`BarAction`].

use std::{
  path::{Path, PathBuf},
  sync::Arc,
  time::{Instant, SystemTime},
};

use orchester_core::id::{ProjectId, RunId};
use orchester_ipc::WorkSummary;
use orchester_workspace::{
  activity::{ActivityLog, OperationId},
  messages::Message,
  runner::{RunnerStatus, state::RunnerState},
  tasks::{data::ProjectTasks, usage::ProjectUsage},
};

use crate::{mcp::McpStatus, routes::Route};

#[derive(Clone, Debug, PartialEq)]
pub struct BarFacts {
  /// The active tab's project, once it is open; `None` on a screen outside a project.
  pub project: Option<ProjectFacts>,
  /// The session's operations and events.
  pub log: ActivityLog,
  pub mcp: McpStatus,
  /// How stopping runs auto-approving goes.
  pub stopping: Stopping,
  /// The team server the item names: the project's, or those this computer is signed in to.
  pub servers: super::server::ServerFacts,
  /// Orchester Cloud, while it cannot be reached.
  pub cloud: super::cloud::CloudFacts,
}

/// How *Stop* of the runs that auto-approve goes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Stopping {
  #[default]
  Idle,
  /// The runs are being told to stop.
  Busy,
  /// They could not be stopped, and why; they still auto-approve.
  Failed(Message),
}

/// The open project the active tab shows.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectFacts {
  pub id: ProjectId,
  pub name: String,
  pub repo_root: PathBuf,
  pub runner: RunnerFacts,
  /// The tasks as they stand with the runner's state; `None` while they are read.
  pub tasks: Option<ProjectTasks>,
  /// The limits and today's usage; `None` while they are read.
  pub usage: Option<ProjectUsage>,
  /// Why the project's tasks could not be read, when they could not.
  pub read_failure: Option<ReadFailure>,
  /// How many runs the previous session left interrupted wait for the person to continue them or not.
  pub undecided_runs: usize,
  /// The accounts the project's connections run on that reported plan limits, by CLI.
  pub limits: Vec<AccountLimits>,
  /// The revision of the project's data: whether its name and folder may show is asked again when it changes.
  pub revision: u64,
}

/// An account a project's connection runs on, with the newest plan reading its CLI reported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountLimits {
  pub account: orchester_core::account::AgentAccount,
  pub reading: orchester_core::connection::plan_limits::LimitReading,
}

/// The project's tasks could not be read, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadFailure {
  /// The first reading failed: no tasks are known.
  First(Message),
  /// Reading them again failed: the tasks shown are the ones last read.
  Again(Message),
}

/// The project's runner as the bar shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct RunnerFacts {
  pub state: RunnerState,
  /// When the next reconnect attempt starts, while reconnecting.
  pub next_attempt: Option<Instant>,
  /// When its current problem began: the drop, or the failed start.
  pub trouble_since: Option<SystemTime>,
  /// What it works on, while it is online or stopping and has said so.
  pub work: Option<WorkSummary>,
  /// Runs whose agents were at work when it dropped, while no runner is back.
  pub interrupted: usize,
}

impl RunnerFacts {
  /// The runner of the project at `repo_root` as `status` reports it; one that has not reported yet is
  /// starting.
  pub fn of(status: Option<&RunnerStatus>, repo_root: &Path) -> Self {
    let Some(status) = status else {
      return Self::starting();
    };
    let state = status.state_for(repo_root);
    Self {
      // While the window closes the runner stops at checkpoints, and what it last reported still counts.
      work: matches!(state, RunnerState::Online | RunnerState::Stopping)
        .then(|| status.work().cloned())
        .flatten(),
      next_attempt: status.until(),
      trouble_since: status.trouble_since(),
      interrupted: status.interrupted(),
      state,
    }
  }

  pub const fn starting() -> Self {
    Self {
      state: RunnerState::Starting,
      next_attempt: None,
      trouble_since: None,
      work: None,
      interrupted: 0,
    }
  }
}

/// What an item of the bar asks the window to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BarAction {
  /// Open the active project's runner log.
  OpenRunnerLog,
  /// Start the active project's runner again.
  RestartRunner,
  /// Go to `route` in the active tab.
  Go(Route),
  /// Stop auto-approving these runs of the active project.
  StopAutoApproving(Vec<RunId>),
  /// A failure or a decision in the activity slot was opened or dismissed: it leaves the slot.
  Resolve(OperationId),
  /// Read the active project's tasks again, after a reading failed.
  ReadAgain,
  /// Open the list of this project's interrupted runs on its Overview.
  ReviewInterrupted(ProjectId),
}

/// Where the bar's actions go.
pub type Dispatch = Arc<dyn Fn(BarAction) + Send + Sync>;
