//! A project's settings as Project settings shows them, one query per stored source, keyed by the project's folder and
//! read through the workspace off the UI thread once reads are released (`ReadHold`).

use std::path::{Path, PathBuf};

use orchester_core::{id::SecretId, memory::MemorySettings, resource::CommandEnvironment};
use orchester_store::StoreError;
use orchester_workspace::{
  background,
  code_index::{self, IndexList, RipgrepCheck},
  connections::data::ProjectConnections,
  display, environment,
  memory::settings as memory,
  messages::{self, Message},
  project_settings::{self as settings_store, RepositoryRead, Settings},
  secrets::data::ProjectSecrets,
  sources::{check::Place, listed::Listed, project as sources},
  workspace::OrchestratorConsent,
};

use super::Session;

/// Reads `read` of the project at `root` off the UI thread once reads are released, saying `what` in the log when it
/// fails.
async fn stored<T: Send + 'static>(
  (Session(services), root): (Session, PathBuf),
  what: &'static str,
  read: impl FnOnce(&crate::services::Services, &Path) -> Result<T, StoreError> + Send + 'static,
) -> Result<T, Message> {
  services.hold.released().await;
  tracing::trace!(target: crate::app_log::READ_LOG, read = what);
  background::run(move || read(&services, &root)).await?.map_err(|error| {
    tracing::error!(%error, read = what, "a project's settings cannot be read");
    messages::store_error(&error)
  })
}

/// The project's settings as General, Runs, *Approved for every run*, Context's agent files and code index show them:
/// limits, agent files, the code index setting, auto mode, Autopilot, the reviewer, the approval rules and base
/// notices.
#[lurq::query(stale_time = "24h", gc_time = "24h")]
pub async fn project_settings(session: Session, root: PathBuf) -> Result<Settings, Message> {
  stored((session, root), "project settings", |_, root| {
    settings_store::load_in(root)
  })
  .await
}

/// The project's stored GitHub link, its connection and the account it expects (General › Repository).
#[lurq::query(stale_time = "24h", gc_time = "24h")]
pub async fn repository(session: Session, root: PathBuf) -> Result<RepositoryRead, Message> {
  stored((session, root), "repository", |_, root| {
    settings_store::repository_in(root)
  })
  .await
}

/// *Let Orchester estimate story points* (Runs › Estimates).
#[lurq::query(stale_time = "24h", gc_time = "24h")]
pub async fn estimates(session: Session, root: PathBuf) -> Result<bool, Message> {
  stored((session, root), "estimates", |services, root| {
    services.workspace.estimates_story_points(root)
  })
  .await
}

/// A switch of what the project's Orchestrator may do itself (Orchestrator).
#[lurq::query(stale_time = "24h", gc_time = "24h")]
pub async fn orchestrator_allows(
  session: Session,
  root: PathBuf,
  consent: OrchestratorConsent,
) -> Result<bool, Message> {
  stored((session, root), "orchestrator switch", move |services, root| {
    services.workspace.orchestrator_allows(root, consent)
  })
  .await
}

/// How agents' records are saved and how many each agent gets (Context › Memory).
#[lurq::query(stale_time = "24h", gc_time = "24h")]
pub async fn memory_settings(session: Session, root: PathBuf) -> Result<MemorySettings, Message> {
  stored((session, root), "memory settings", |_, root| memory::load_in(root)).await
}

/// The variables the project adds for its agents and command tools (Environment).
#[lurq::query(stale_time = "24h", gc_time = "24h")]
pub async fn variables(session: Session, root: PathBuf) -> Result<CommandEnvironment, Message> {
  stored((session, root), "variables", |_, root| environment::load_in(root)).await
}

/// The project's connections and MCP servers (Connections), with their last checks as stored.
#[lurq::query(stale_time = "24h", gc_time = "24h")]
pub async fn connections(session: Session, root: PathBuf) -> Result<ProjectConnections, Message> {
  stored((session, root), "connections", |_, root| {
    ProjectConnections::load_in(root)
  })
  .await
}

/// The project's secrets, metadata only, with the recent access of the one `selected` shows (Environment): live, since
/// what uses each and when it was read change with the project's runs and connections.
#[lurq::query(stale_time = "0s", gc_time = "24h")]
pub async fn secrets(session: Session, root: PathBuf, selected: Option<SecretId>) -> Result<ProjectSecrets, Message> {
  stored((session, root), "secrets", move |_, root| {
    ProjectSecrets::load_selected_in(root, selected)
  })
  .await
}

/// The project's sources with whether runs can read each now, where a new one is checked, and when they were read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourcesRead {
  pub listed: Vec<Listed>,
  pub place: Place,
  pub checked: String,
}

/// Context › Sources: live, since whether a source can be read is this computer's state.
#[lurq::query(stale_time = "0s", gc_time = "24h")]
pub async fn project_sources(session: Session, root: PathBuf) -> Result<SourcesRead, Message> {
  stored((session, root), "sources", |_, root| {
    Ok(SourcesRead {
      listed: sources::load_in(root)?,
      place: sources::place_in(root)?,
      checked: display::clock(orchester_core::time::Timestamp::now()),
    })
  })
  .await
}

/// The code index's builds of the project (Context › Code index): live, since a build runs on its own. Not held with
/// the settings' reads: the tab's bones stand for the setting only.
#[lurq::query(stale_time = "0s", gc_time = "24h")]
pub async fn index_builds(_session: Session, root: PathBuf) -> Result<IndexList, Message> {
  tracing::trace!(target: crate::app_log::READ_LOG, read = "code index");
  background::run(move || code_index::list_in(&root)).await?
}

/// Whether ripgrep, the code index's search tool, is installed for the project (Context › Code index): live.
#[lurq::query(stale_time = "0s", gc_time = "24h")]
pub async fn ripgrep(_session: Session, root: PathBuf) -> Result<RipgrepCheck, Message> {
  background::run(move || code_index::check_ripgrep(&root)).await?
}
