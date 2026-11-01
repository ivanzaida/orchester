//! What the status bar shows, as plain data: the active tab's project with its runner and tasks, the
//! session's activity, and the app's MCP server. The window gathers it ([`super::StatusBar`]); a test
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
  tasks::data::ProjectTasks,
};

use crate::{mcp::McpStatus, navigation::press::OpenIn, routes::Route, ui::pressable::Press};

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
  /// This copy's update, while it needs the person or has news.
  pub update: super::update::UpdateFacts,
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
  pub repo_root: PathBuf,
  pub runner: RunnerFacts,
  /// The tasks as they stand with the runner's state; `None` while they are read.
  pub tasks: Option<ProjectTasks>,
  /// Why the project's tasks could not be read, when they could not.
  pub read_failure: Option<ReadFailure>,
  /// How many runs the previous session left interrupted wait for the person to continue them or not.
  pub undecided_runs: usize,
  /// The accounts the project's connections run on that reported plan limits, by CLI.
  pub limits: Vec<AccountLimits>,
  /// How many tabs the project's browser has.
  pub browser_tabs: usize,
}

/// An account a project's connection runs on, with the newest plan reading its CLI reported, and where that came from:
/// a turn, or a read outside a run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountLimits {
  pub account: orchester_core::account::AgentAccount,
  pub reading: orchester_core::connection::plan_limits::LimitReading,
  pub source: orchester_core::connection::plan_limits::ReadingSource,
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
  /// When it came online, while it is online or stopping.
  pub online_since: Option<SystemTime>,
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
    // While the window closes the runner stops at checkpoints, and what it last reported still counts.
    let attached = matches!(state, RunnerState::Online | RunnerState::Stopping);
    Self {
      work: attached.then(|| status.work().cloned()).flatten(),
      next_attempt: status.until(),
      trouble_since: status.trouble_since(),
      online_since: attached.then(|| status.online_since()).flatten(),
      interrupted: status.interrupted(),
      state,
    }
  }

  pub const fn starting() -> Self {
    Self {
      state: RunnerState::Starting,
      next_attempt: None,
      trouble_since: None,
      online_since: None,
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
  /// Open `route` where the press asks (`navigation::link`).
  Go(Route, OpenIn),
  /// Stop auto-approving these runs of the active project.
  StopAutoApproving(Vec<RunId>),
  /// A failure or a decision in the activity slot was opened or dismissed: it leaves the slot.
  Resolve(OperationId),
  /// Read the active project's tasks again, after a reading failed.
  ReadAgain,
  /// Open the list of this project's interrupted runs on its Overview.
  ReviewInterrupted(ProjectId),
  /// Open the Browser window on the active project.
  ShowBrowser,
}

/// Where the bar's actions go.
pub type Dispatch = Arc<dyn Fn(BarAction) + Send + Sync>;

/// The press of an item that opens `route` through `dispatch`, where the press asks.
pub fn go(dispatch: &Dispatch, route: Route) -> Press {
  let dispatch = dispatch.clone();
  Press::Open(Arc::new(move |at| dispatch(BarAction::Go(route.clone(), at))))
}
