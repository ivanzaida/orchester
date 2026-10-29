//! What a started session works with, shared by the window and the app's MCP tools: the workspace and
//! its projects, the projects' runners, the session's background activity, the programs git work runs,
//! the clock, the keychain, the shared servers and Orchester Cloud, where web pages open, what automation changed and
//! is merging or removing, and the window's close flow and the start-up recovery of the runs the previous session left
//! interrupted. They come from `orchester-workspace`, which the deprecated desktop shares; this app only puts them
//! together ([`Services::start`]) and hands them to its screens ([`Services::of`]).
//!
//! The shared servers' connection monitor, the shared projects' sync and the check of this computer's cloud sign-in
//! run on the app's runtime while [`Background`] is held, which `main` keeps until the window closes.

use std::path::PathBuf;

use lurq::app::ctx::Ctx;
use orchester_core::time::Timestamp;
use orchester_exec::programs::Programs;
use orchester_ipc::SessionEnd;
use orchester_workspace::{
  activity::Activity,
  clock::Clock,
  close::CloseFlow,
  plan_limits::reads::PlanReads,
  recovery::Recovery,
  runner::Runners,
  secrets::Vault,
  session::AppSession,
  sharing::{self, Servers, cloud::Cloud, team_join::TeamJoin},
  tasks::{integration_jobs::ProjectJobs, outside::OutsideChanges, resolve::Editor},
  workspace::Workspace,
};
use tokio::{runtime::Handle, task::JoinHandle};

pub mod accounts;
pub mod attachment_picker;
pub mod links;
pub mod path_picker;
pub mod plan_limits;
pub mod reference_picker;

use self::links::Links;
use crate::{mcp::chat::ChatServers, project::hold::ReadHold};

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
  /// This computer's Orchester Cloud account and its invites.
  pub cloud: Cloud,
  /// Joining a team, by the Cloud account or by a server's address and a code, with no project open.
  pub team_join: TeamJoin,
  /// Where web pages open: the browser, or a record in a test or a QA copy.
  pub links: Links,
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
  /// What closing the window does with the agents at work.
  pub close: CloseFlow,
  /// The runs the previous session left interrupted, and what the person decided for them.
  pub recovery: Recovery,
  /// The servers of the Orchestrator chat's domain tools, one per conversation that sent a message.
  pub chat_tools: ChatServers,
  /// The editor *Resolve in editor* opens a run's worktree in, found as the session starts; `None` where there is
  /// none.
  pub editor: Option<Editor>,
  /// The reads of the accounts' plan limits outside runs; they read only once the app's own window started them
  /// (`Launch::read_plan_limits`), never in a test or a QA copy.
  pub plan_reads: PlanReads,
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
  /// `runtime`, with `vault` (the keychain, or one in memory in a test) and real processes.
  pub fn start(session: &AppSession, runner: PathBuf, runtime: Handle, vault: Vault) -> Self {
    let activity = Activity::new();
    let workspace = session.workspace().clone();
    let runners = Runners::new(runner, runtime.clone(), activity.clone(), session.previous_end());
    let programs = Programs::processes();
    let jobs = ProjectJobs::default();
    let recovery = Recovery::start(
      (workspace.clone(), programs.clone()),
      session.previous_end(),
      session.interrupted_since(),
      &runtime,
    );
    let servers = Servers::new(workspace.clone(), vault.clone());
    let cloud = Cloud::new(workspace.clone(), vault.clone(), servers.clone());
    Self {
      close: CloseFlow::new(runners.clone(), jobs.clone(), runtime.clone()),
      recovery,
      plan_reads: PlanReads::new(workspace.clone(), runners.clone()),
      runners,
      team_join: TeamJoin::new(servers.clone(), cloud.clone()),
      cloud,
      links: Links::from_env(),
      servers,
      workspace,
      activity,
      programs,
      clock: Clock::system(),
      vault,
      changes: OutsideChanges::default(),
      jobs,
      hold: ReadHold::default(),
      runtime,
      previous_end: session.previous_end(),
      session_start: session.started_at(),
      chat_tools: ChatServers::default(),
      editor: Editor::find(),
    }
  }

  /// The services the root provides to everything it mounts once the session started.
  pub fn of(ctx: &mut Ctx) -> Self {
    ctx
      .use_context::<Self>()
      .expect("the root provides the services before mounting the window of a started session")
  }

  /// Starts the shared servers' connection monitor, the shared projects' sync, the check of the cloud sign-in
  /// (`sharing::cloud::keep_checking`) and the activity's following of the account signed in; they run until the
  /// returned [`Background`] is dropped.
  pub fn run_background(&self) -> Background {
    Background(vec![
      self.runtime.spawn(sharing::monitor(self.servers.clone())),
      self.runtime.spawn(sharing::project::work::run(
        self.servers.clone(),
        self.runners.run_changes(),
        self.programs.clone(),
      )),
      self.runtime.spawn(sharing::cloud::keep_checking(self.cloud.clone())),
      self.runtime.spawn(sharing::cloud::keep_activity_to_the_account(
        self.cloud.clone(),
        self.activity.clone(),
      )),
    ])
  }
}

/// The session's background tasks; dropping it ends them.
pub struct Background(Vec<JoinHandle<()>>);

impl Background {
  /// Ends `task` with the others.
  pub fn hold(&mut self, task: JoinHandle<()>) {
    self.0.push(task);
  }
}

impl Drop for Background {
  fn drop(&mut self) {
    for task in &self.0 {
      task.abort();
    }
  }
}
