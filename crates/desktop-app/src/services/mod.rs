//! What a started session works with, shared by the window and the app's MCP tools: the workspace and
//! its projects, the projects' runners, the session's background activity, the programs git work runs,
//! the clock, the keychain, the shared servers, and what automation changed and is merging or removing.
//! They come from `orchester-workspace`, which the deprecated desktop shares; this app only puts them
//! together ([`Services::start`]) and hands them to its screens ([`Services::of`]).
//!
//! The shared servers' connection monitor and the shared projects' sync run on the app's runtime while
//! [`Background`] is held, which `main` keeps until the window closes.

use std::path::PathBuf;

use lurq::app::ctx::Ctx;
use orchester_core::time::Timestamp;
use orchester_exec::programs::Programs;
use orchester_ipc::SessionEnd;
use orchester_workspace::{
  activity::Activity,
  clock::Clock,
  runner::Runners,
  secrets::Vault,
  session::AppSession,
  sharing::{self, Servers},
  tasks::{integration_jobs::ProjectJobs, outside::OutsideChanges},
  workspace::Workspace,
};
use tokio::{runtime::Handle, task::JoinHandle};

use crate::project::hold::ReadHold;

/// The session's services. Every field is a handle: clones share the same workspace, runners and
/// operations.
#[derive(Clone)]
pub struct Services {
  pub workspace: Workspace,
  pub runners: Runners,
  /// The session's background operations and events, for the status bar.
  pub activity: Activity,
  /// How git and `gh` run for merges, removals and pull request sync.
  pub programs: Programs,
  /// The time live values are shown at.
  pub clock: Clock,
  /// Where secret values are kept: the OS keychain.
  pub vault: Vault,
  pub servers: Servers,
  /// What automation changed in which project, so only the screens that show it read it again.
  pub changes: OutsideChanges,
  /// The projects' merge and worktree-removal operations, shared by the task pages and the MCP tools.
  pub jobs: ProjectJobs,
  /// Holds the project feeds' reads while QA captures loading states.
  pub hold: ReadHold,
  /// The runtime background work runs on.
  pub runtime: Handle,
  /// How the previous session ended, as the runners are told when they attach.
  pub previous_end: SessionEnd,
  /// When this session started.
  pub session_start: Timestamp,
}

/// Services are the session's: two are equal when they share its workspace.
impl PartialEq for Services {
  fn eq(&self, other: &Self) -> bool {
    self.workspace == other.workspace
  }
}

impl lurq::app::component::DevtoolsInspectable for Services {}

impl Services {
  /// The services of `session`, whose runners are `runner` (the program beside this one), on
  /// `runtime`, with the keychain and real processes.
  pub fn start(session: &AppSession, runner: PathBuf, runtime: Handle) -> Self {
    let activity = Activity::new();
    let workspace = session.workspace().clone();
    let vault = Vault::keychain();
    Self {
      runners: Runners::new(runner, runtime.clone(), activity.clone(), session.previous_end()),
      servers: Servers::new(workspace.clone(), vault.clone()),
      workspace,
      activity,
      programs: Programs::processes(),
      clock: Clock::system(),
      vault,
      changes: OutsideChanges::default(),
      jobs: ProjectJobs::default(),
      hold: ReadHold::default(),
      runtime,
      previous_end: session.previous_end(),
      session_start: session.started_at(),
    }
  }

  /// The services the root provides to everything it mounts once the session started.
  pub fn of(ctx: &mut Ctx) -> Self {
    ctx
      .use_context::<Self>()
      .expect("the root provides the services before mounting the window of a started session")
  }

  /// Starts the shared servers' connection monitor and the shared projects' sync; they run until the
  /// returned [`Background`] is dropped.
  pub fn run_background(&self) -> Background {
    Background(vec![
      self.runtime.spawn(sharing::monitor(self.servers.clone())),
      self.runtime.spawn(sharing::project::work::run(
        self.servers.clone(),
        self.runners.run_changes(),
      )),
    ])
  }
}

/// The session's background tasks; dropping it ends them.
pub struct Background(Vec<JoinHandle<()>>);

impl Drop for Background {
  fn drop(&mut self) {
    for task in &self.0 {
      task.abort();
    }
  }
}
