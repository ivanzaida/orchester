//! The tools on a team project (`docs/architecture/team-authority-plan.md` §4 and §10, milestone P7): what the team
//! server holds is read from it ([`crate::team_reads`]) and changed through the dispatcher ([`crate::actions`]), as on
//! the screens, a team task's work starts through the team server ([`crate::team_work`]), and `merge_run` of its run
//! requests its module's delivery ([`deliver`]). Nothing falls back to the project database's copy of team tasks or
//! modules: a server that cannot be reached, a signed-out session or a refusal is the tool's refusal. A project without
//! a link to a server is local, and its tools keep their own paths.

mod bundle_missing;
mod deliver;
mod modules;
mod outcome;
mod run;
mod tasks;
mod writes;

use orchester_store::{ProjectDb, SharingLink};

pub(super) use self::{
  deliver::merge_run,
  modules::{create_module, get_module, list_modules, update_module},
  run::run_task,
  tasks::{get_task, list_tasks},
  writes::{create_task, update_task},
};
use crate::{
  actions::Dispatcher,
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  messages::Message,
  sharing::{SharingError, sync::SystemClock},
  team_reads::{TeamProject, TeamReadError, TeamSession},
  workspace::OpenProject,
};

/// A team project the tools act on: its folder and the server project it is linked to.
pub(super) struct Team {
  pub(super) open: OpenProject,
  link: SharingLink,
}

/// The [`Team`] of project `named` when it is linked to a team server; `None` for a local project.
pub(super) async fn of(services: &Services, named: &str) -> Result<Option<Team>, ToolError> {
  let (services, named) = (services.clone(), named.to_owned());
  blocking(move || {
    let open = services.project(&named)?;
    let link = ProjectDb::open(&open.repo_root)?.sharing()?;
    Ok(link.map(|link| Team { open, link }))
  })
  .await
}

impl Team {
  /// A session of the team server, signed in for the project's link; refused when it cannot be made.
  pub(super) async fn session(&self, services: &Services) -> Result<TeamSession, ToolError> {
    services
      .servers
      .team_session(self.link.server, self.link.project)
      .await
      .map_err(|error| session_refused(&error))
  }

  /// The project's reads in a session of its own.
  pub(super) async fn reads(&self, services: &Services) -> Result<TeamProject, ToolError> {
    Ok(TeamProject::new(self.session(services).await?))
  }

  /// The project's dispatcher in a session of its own.
  pub(super) async fn dispatcher(&self, services: &Services) -> Result<Dispatcher, ToolError> {
    let session = self.session(services).await?;
    Ok(Dispatcher::new(
      self.open.repo_root.clone(),
      services.servers.clone(),
      Some(session),
    ))
  }
}

/// `read` of `project`, while the project's subscription gives the reads their position: one call's reads, never a
/// copy kept here. The subscription ends with the read.
pub(super) async fn read<T>(
  project: &TeamProject,
  read: impl Future<Output = Result<T, TeamReadError>>,
) -> Result<T, TeamReadError> {
  tokio::select! {
    ended = project.follow(&SystemClock) => Err(ended),
    read = read => read,
  }
}

/// Runs the future `work` makes on a worker of the blocking pool, where it may block: the dispatcher's and a team
/// start's futures hold the project database across their awaits, so they are not `Send`, while a tool's future is.
pub(super) async fn on_worker<T, F>(work: impl FnOnce() -> F + Send + 'static) -> Result<T, ToolError>
where
  F: Future<Output = Result<T, ToolError>>,
  T: Send + 'static,
{
  let runtime = tokio::runtime::Handle::current();
  blocking(move || runtime.block_on(work())).await
}

/// A team read's failure as the tool's refusal; a caller names what was not found itself.
pub(super) fn read_refused(error: &TeamReadError) -> ToolError {
  let (code, key) = match error {
    TeamReadError::NeedsVersion { required } => return outcome::needs_version(required),
    TeamReadError::Unavailable { .. } | TeamReadError::KeptChanging => {
      ("team_unavailable", "appMcp.errors.teamUnavailable")
    }
    TeamReadError::SignedOut | TeamReadError::SessionChanged => ("team_signed_out", "appMcp.errors.teamSignedOut"),
    TeamReadError::MembershipRevoked => ("team_membership_revoked", "appMcp.errors.teamMembershipRevoked"),
    TeamReadError::NotFound => ("team_not_found", "appMcp.errors.teamNotFound"),
    TeamReadError::CursorExpired | TeamReadError::Refused(_) | TeamReadError::Server(_) | TeamReadError::Unreadable => {
      ("team_read_failed", "appMcp.errors.teamReadFailed")
    }
  };
  tracing::warn!(%error, "a tool's team read failed");
  ToolError::refused(code, vec![Message::new(key)])
}

/// No session of the project's server could be made.
fn session_refused(error: &SharingError) -> ToolError {
  tracing::warn!(%error, "no team session for a tool");
  match error {
    SharingError::SignedOut => ToolError::refused("team_signed_out", vec![Message::new("appMcp.errors.teamSignedOut")]),
    _ => ToolError::refused("team_unavailable", vec![Message::new("appMcp.errors.teamUnavailable")]),
  }
}

/// Whether the project of `db` is linked to a team server: for a tool that works inside the project's database.
pub(super) fn linked(db: &ProjectDb) -> Result<bool, ToolError> {
  Ok(db.sharing()?.is_some())
}

/// A tool whose subject the team server owns and this version does not read or change there yet: refused for a team
/// project rather than answered from this computer's copy.
pub(super) fn team_only_on_server(tool: &str) -> ToolError {
  ToolError::refused(
    "team_project_unsupported",
    vec![Message::new("appMcp.errors.teamProjectUnsupported").with("tool", tool.to_owned())],
  )
}

/// A change whose owner is the open team-authority decision `decision`: refused before anything is sent.
pub(super) fn decision_required(decision: orchester_api::v2::DecisionId) -> ToolError {
  outcome::action_refused(crate::actions::ActionError::DecisionRequired(decision))
}

/// A team task's run is never merged into the base branch from here: its work goes into its module's branch, and only
/// the module's delivery takes it further, which `merge_run` requests ([`merge_run`]). A merge a person approved in a
/// proposal is of the run's own commit, so on a team project it is refused.
pub(super) fn merge_refused() -> ToolError {
  ToolError::refused(
    "team_delivery_pending",
    vec![Message::new("appMcp.errors.teamDeliveryPending")],
  )
}

/// The team task `key` names (its key or its server id) as the team server reads it, when `project` is a team
/// project; `None` for a local one. For a tool that lists what this computer recorded of the task: the server says
/// which task it is, and whether it exists, so a task never run here is not mistaken for one that does not exist.
pub(super) async fn read_task(
  services: &Services,
  project: &str,
  key: &str,
) -> Result<Option<orchester_api::v2::TaskRead>, ToolError> {
  let Some(team) = of(services, project).await? else {
    return Ok(None);
  };
  let reads = team.reads(services).await?;
  tasks::find(&reads, key).await.map(Some)
}
