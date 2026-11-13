//! What the Agents hub shows, one query per read, keyed by the project's folder and read through the workspace off the
//! UI thread once reads are released (`ReadHold`): the hub of a team project and of a local one, which are live (a
//! tool's or a server's secret is looked up in the keychain, a program on this computer), and what a local
//! instruction's, a local skill's, a local tool's and a new profile's panel edit over, which are stored.

use std::path::{Path, PathBuf};

use orchester_core::{
  id::ResourceId,
  resource::{Instruction, ResourceBody, Skill, Tool},
};
use orchester_workspace::{
  agents::{
    data::ProjectAgents,
    local_given::{GivenLists, offered_in},
    local_hub::LocalHub,
    team_hub::TeamHubRead,
  },
  background,
  instructions::data::ProjectInstructions,
  messages::{self, Message},
  pipelines::{default::default_in, hub::PipelineHub},
  removal,
  skills::editor::EditorData,
  tools::data::ProjectTools,
};

use super::Session;
use crate::services::Services;

/// Reads `read` off the UI thread once reads are released.
async fn read<T: Send + 'static>(
  Session(services): Session,
  read: impl FnOnce(&Services) -> Result<T, Message> + Send + 'static,
) -> Result<T, Message> {
  services.hold.released().await;
  background::run(move || read(&services)).await?
}

/// A team project's hub: the team's resources as its team server was read this session and how each stands on this
/// computer, or that the server is being read or cannot be reached. Live: what this computer filled in
/// is looked up as it is read, so each visit shows the last answer and reads it again behind it. The team's
/// configuration is not in the key, since the hub's read does not take it: a change of it invalidates this query
/// (`follow::ProjectReads`), and the screen lists the answer with the configuration it has then.
#[lurq::query(stale_time = "0s", gc_time = "24h")]
pub async fn team_hub(session: Session, root: PathBuf) -> Result<TeamHubRead, Message> {
  tracing::trace!(target: crate::app_log::READ_LOG, read = "team_hub");
  read(session, move |services| {
    services
      .workspace
      .team_hub(&root, services.vault.get())
      .map_err(|error| error.message())
  })
  .await
}

/// A local project's hub: its resources and how each stands on this computer. Live, as [`team_hub`].
#[lurq::query(stale_time = "0s", gc_time = "24h")]
pub async fn local_hub(session: Session, root: PathBuf) -> Result<LocalHub, Message> {
  tracing::trace!(target: crate::app_log::READ_LOG, read = "local_hub");
  read(session, move |services| {
    services
      .workspace
      .local_hub(&root, services.vault.get())
      .map_err(|error| error.message())
  })
  .await
}

/// What the Pipelines hub shows: the project's pipelines and the one New task offers first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipelineHubRead {
  pub hub: PipelineHub,
  /// The project's default pipeline, if a person chose one; it may be archived.
  pub default: Option<ResourceId>,
}

/// The Pipelines hub of a local or a team project: its pipelines, the tasks that use them and how each stands on this
/// computer, and its default pipeline. Live, as [`team_hub`]: what a pipeline runs is ready on this computer or not as
/// the keychain and the disk say.
#[lurq::query(stale_time = "0s", gc_time = "24h")]
pub async fn pipeline_hub(session: Session, root: PathBuf) -> Result<PipelineHubRead, Message> {
  tracing::trace!(target: crate::app_log::READ_LOG, read = "pipeline_hub");
  read(session, move |services| {
    let hub = services
      .workspace
      .pipeline_hub(&root, services.vault.get())
      .map_err(|error| error.message())?;
    let default = default_in(&root).map_err(|error| messages::store_error(&error))?;
    Ok(PipelineHubRead { hub, default })
  })
  .await
}

/// The project's instructions with the profiles and pipelines they can name, and the published version of each that
/// has a draft, which a local instruction's panel starts from and compares with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstructionsRead {
  pub data: ProjectInstructions,
  pub published: Vec<(ResourceId, Instruction)>,
}

/// What a local instruction's panel edits over: one read for every instruction of the project.
#[lurq::query(stale_time = "24h", gc_time = "24h")]
pub async fn instructions(session: Session, root: PathBuf) -> Result<InstructionsRead, Message> {
  read(session, move |_| {
    let data = ProjectInstructions::load_in(&root).map_err(|error| messages::store_error(&error))?;
    let drafted = data.entries.iter().filter(|entry| entry.state.has_draft());
    let published = published_of(&root, drafted.map(|entry| entry.id))?;
    Ok(InstructionsRead { data, published })
  })
  .await
}

/// What the Skills editor checks a write against (the project's skills, the scopes they can take), and the published
/// version of each skill that has a draft, which a local skill's panel starts from and compares with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillsRead {
  pub data: EditorData,
  pub published: Vec<(ResourceId, Skill)>,
}

/// What a local skill's panel edits over: one read for every skill of the project.
#[lurq::query(stale_time = "24h", gc_time = "24h")]
pub async fn skills(session: Session, root: PathBuf) -> Result<SkillsRead, Message> {
  read(session, move |_| {
    let data = EditorData::stored_in(&root)?;
    let drafted = data.listed.entries.iter().filter(|entry| entry.state.has_draft());
    let published = published_of(&root, drafted.map(|entry| entry.id))?;
    Ok(SkillsRead { data, published })
  })
  .await
}

/// The project's tools, and the published version of each that has a draft, which a local tool's panel starts from and
/// compares with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolsRead {
  pub listed: ProjectTools,
  pub published: Vec<(ResourceId, Tool)>,
}

/// What a local tool's panel edits over: one read for every tool of the project.
#[lurq::query(stale_time = "24h", gc_time = "24h")]
pub async fn tools(session: Session, root: PathBuf) -> Result<ToolsRead, Message> {
  read(session, move |_| {
    let listed = ProjectTools::load_in(&root).map_err(|error| messages::store_error(&error))?;
    let drafted = listed.tools.iter().filter(|entry| entry.state.has_draft());
    let published = published_of(&root, drafted.map(|entry| entry.id))?;
    Ok(ToolsRead { listed, published })
  })
  .await
}

/// The published body of each of `drafted` that has one.
fn published_of<T: ResourceBody + Clone>(
  root: &Path,
  drafted: impl Iterator<Item = ResourceId>,
) -> Result<Vec<(ResourceId, T)>, Message> {
  let mut published = Vec::new();
  for id in drafted {
    if let Some(body) = removal::published_in::<T>(root, id)? {
      published.push((id, body));
    }
  }
  Ok(published)
}

/// What the form that makes a profile chooses from: the project's agent connections (its CLI and model choices) and
/// MCP servers, and the project's instructions, skills and tools it can give its agents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileOffer {
  pub agents: ProjectAgents,
  pub given: GivenLists,
}

/// [`ProfileOffer`] of the project at `root`. Not held with the hub's reads: the form shows its fields before they are
/// read.
#[lurq::query(stale_time = "24h", gc_time = "24h")]
pub async fn profile_form(_session: Session, root: PathBuf) -> Result<ProfileOffer, Message> {
  background::run(move || {
    let agents = ProjectAgents::load_in(&root).map_err(|error| messages::store_error(&error))?;
    let given = offered_in(&root).map_err(|error| messages::store_error(&error))?;
    Ok(ProfileOffer { agents, given })
  })
  .await?
}
