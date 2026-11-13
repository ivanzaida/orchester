//! Reading one pipeline through the project database, and what its steps
//! can be assigned to. Every function blocks on SQLite; the UI calls them off the UI thread.

use std::{path::Path, sync::Arc};

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, Connection, ConnectionKind,
    mcp::{McpServer, McpServerSelection},
  },
  id::ResourceId,
  permission::AgentPermissions,
  resource::{AgentProfile, ModelId, Pipeline, ResourceKind, Tool, ToolAction, ToolKind, Version, VersionNumber},
  store::{ConnectionStore, McpServerStore, ResourceStore, TaskStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use crate::{
  import::imported::{Imported, imported},
  readiness::ReadinessStanding,
};

/// A published agent profile a step can run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileChoice {
  pub id: ResourceId,
  pub name: Arc<str>,
  pub version: VersionNumber,
  /// The profile's model, which steps inherit unless they override it.
  pub model: ModelId,
  /// Name of the connection that runs the profile; `None` if it was removed.
  pub connection: Option<Arc<str>>,
  /// The CLI of that connection, which decides what a step can narrow the permissions to.
  pub provider: Option<AgentProvider>,
  /// The permissions steps narrow.
  pub permissions: AgentPermissions,
  /// That connection, when it is an agent CLI: which models it cannot run.
  pub agent: Option<AgentConnection>,
  /// The MCP servers the profile gives its agents.
  pub mcp: McpServerSelection,
}

/// Display-only draft profile. It is never returned by the published-profile lookup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DraftProfileChoice {
  pub id: ResourceId,
  pub name: Arc<str>,
  pub model: ModelId,
}

/// Display-only draft tool: one with no published version yet. It is never returned by the published-tool lookup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DraftToolChoice {
  pub id: ResourceId,
  pub name: Arc<str>,
}

/// A published tool a step can call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolChoice {
  pub id: ResourceId,
  pub name: Arc<str>,
  pub version: VersionNumber,
  pub kind: ToolKind,
  /// The command line of a command tool.
  pub command: Option<Arc<str>>,
  /// Whether a step can run it in this version: a command tool without access rules.
  pub runnable: bool,
}

impl ProfileChoice {
  /// Profile `id` named `name`, as its version `number` with `body` runs on one of `connections`.
  pub(crate) fn of(
    (id, name): (ResourceId, &str),
    number: VersionNumber,
    body: &AgentProfile,
    connections: &[Connection],
  ) -> Self {
    let connection = connections
      .iter()
      .find(|connection| connection.id() == body.connection());
    let provider = connection.and_then(|connection| match connection.kind() {
      ConnectionKind::Agent(provider) => Some(*provider),
      ConnectionKind::GitHub { .. } => None,
    });
    Self {
      connection: connection.map(|connection| connection.name().as_str().into()),
      provider,
      permissions: body.permissions().clone(),
      // A connection that is no agent CLI (GitHub) narrows no models: it has none to run.
      agent: connection.and_then(|connection| AgentConnection::try_from(connection.clone()).ok()),
      id,
      name: name.into(),
      version: number,
      model: body.model().clone(),
      mcp: body.mcp_servers().clone(),
    }
  }
}

impl ToolChoice {
  /// Tool `id` named `name`, as its version `number` with `tool` reads.
  pub(crate) fn of((id, name): (ResourceId, &str), number: VersionNumber, tool: &Tool) -> Self {
    Self {
      id,
      name: name.into(),
      version: number,
      kind: tool.kind(),
      command: match tool.action() {
        ToolAction::Command(command) => Some(command.command_line().into()),
        ToolAction::Http(_) | ToolAction::DatabaseQuery(_) => None,
      },
      runnable: tool.kind().runnable() && tool.access().is_none(),
    }
  }
}

/// What steps of the project's pipelines can be assigned to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Assignables {
  pub profiles: Vec<ProfileChoice>,
  pub draft_profiles: Vec<DraftProfileChoice>,
  pub tools: Vec<ToolChoice>,
  pub draft_tools: Vec<DraftToolChoice>,
  /// The project's MCP servers, which say what the profiles' servers run without asking.
  pub mcp_servers: Vec<McpServer>,
  /// How each published profile and tool stands on this computer, when the caller read the project's readiness
  /// ([`crate::pipelines::hub`]); empty otherwise.
  pub standings: Vec<(ResourceId, ReadinessStanding)>,
}

impl Assignables {
  pub fn load(store: &ProjectDb) -> Result<Self, StoreError> {
    let connections = store.connections()?;
    let mut profiles = Vec::new();
    let mut draft_profiles = Vec::new();
    for resource in store.resources(ResourceKind::AgentProfile)? {
      if let Some(version) = store.latest_version::<AgentProfile>(resource.id())? {
        let named = (resource.id(), resource.name().as_str());
        let here = store.profile_here(resource.id(), version.body().clone())?;
        profiles.push(ProfileChoice::of(named, version.number(), &here, &connections));
      } else if let Some(draft) = store.draft::<AgentProfile>(resource.id())? {
        draft_profiles.push(DraftProfileChoice {
          id: resource.id(),
          name: resource.name().as_str().into(),
          model: draft.body().model().clone(),
        });
      }
    }
    let mut tools = Vec::new();
    let mut draft_tools = Vec::new();
    for resource in store.resources(ResourceKind::Tool)? {
      if let Some(version) = store.latest_version::<Tool>(resource.id())? {
        tools.push(ToolChoice::of(
          (resource.id(), resource.name().as_str()),
          version.number(),
          version.body(),
        ));
      } else if store.draft::<Tool>(resource.id())?.is_some() {
        draft_tools.push(DraftToolChoice {
          id: resource.id(),
          name: resource.name().as_str().into(),
        });
      }
    }
    Ok(Self {
      profiles,
      draft_profiles,
      tools,
      draft_tools,
      mcp_servers: store.mcp_servers()?,
      standings: Vec::new(),
    })
  }

  /// The servers `profile`'s agents get that run tools without asking, with how many.
  pub fn exempt_tools(&self, profile: &ProfileChoice) -> Vec<(Arc<str>, usize)> {
    profile
      .mcp
      .exempt_tools(&self.mcp_servers)
      .into_iter()
      .map(|(server, tools)| (server.as_str().into(), tools.len()))
      .collect()
  }

  /// How the profile or tool `id` stands on this computer; `None` when the readiness was not read, or it is unknown.
  pub fn standing(&self, id: ResourceId) -> Option<&ReadinessStanding> {
    self
      .standings
      .iter()
      .find(|(known, _)| *known == id)
      .map(|(_, standing)| standing)
  }

  pub fn profile(&self, id: ResourceId) -> Option<&ProfileChoice> {
    self.profiles.iter().find(|profile| profile.id == id)
  }

  pub fn tool(&self, id: ResourceId) -> Option<&ToolChoice> {
    self.tools.iter().find(|tool| tool.id == id)
  }

  /// Whether the profile or tool `id` is gone here: neither published nor a draft, so it was archived or removed. A
  /// step that names it has to run something else.
  pub fn gone(&self, id: ResourceId) -> bool {
    self.profile(id).is_none()
      && self.tool(id).is_none()
      && !self.draft_profiles.iter().any(|draft| draft.id == id)
      && !self.draft_tools.iter().any(|draft| draft.id == id)
  }

  /// The published profile named `name` (ignoring case), if exactly one has that name.
  pub fn profile_named(&self, name: &str) -> Option<ResourceId> {
    single(self.profiles.iter().map(|profile| (profile.id, &*profile.name)), name)
  }

  /// The published tool named `name` (ignoring case), if exactly one has that name.
  pub fn tool_named(&self, name: &str) -> Option<ResourceId> {
    single(self.tools.iter().map(|tool| (tool.id, &*tool.name)), name)
  }
}

fn single<'a>(mut named: impl Iterator<Item = (ResourceId, &'a str)>, name: &str) -> Option<ResourceId> {
  let wanted = name.trim().to_lowercase();
  let found = named.find(|(_, candidate)| candidate.to_lowercase() == wanted)?;
  named
    .all(|(_, candidate)| candidate.to_lowercase() != wanted)
    .then_some(found.0)
}

/// A pipeline as the editor opens it: its latest version, which a save publishes the next one over
/// ([`super::write::save_named_in`]). A pipeline is stored only as published versions (D-032).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenedPipeline {
  pub id: ResourceId,
  pub name: Arc<str>,
  pub slug: Arc<str>,
  /// The latest version's steps.
  pub body: Pipeline,
  /// The latest version, which an edit is made from.
  pub version: VersionNumber,
  /// When it was published.
  pub published_at: Timestamp,
  /// Tasks that use the pipeline.
  pub used_by: usize,
  pub assignables: Arc<Assignables>,
  /// The import that created the pipeline; `None` when it was not imported.
  pub imported: Option<Arc<Imported>>,
  /// The team's pipeline in a shared project: a save goes to the team server and is kept only once it took it
  /// ([`super::write::save_named_in`]).
  pub team: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum OpenError {
  #[error(transparent)]
  Store(#[from] StoreError),
  /// The pipeline was removed, or has no version used here.
  #[error("the pipeline does not exist")]
  Missing,
  /// A team project's configuration is being read, or could not be read this session; nothing older is opened.
  #[error(transparent)]
  TeamNotRead(#[from] crate::team_config::TeamNotRead),
}

impl OpenedPipeline {
  pub fn load(db: &ProjectDb, id: ResourceId) -> Result<Self, OpenError> {
    let resource = db.resource(id)?.ok_or(OpenError::Missing)?;
    let version: Version<Pipeline> = db.latest_version::<Pipeline>(id)?.ok_or(OpenError::Missing)?;
    Ok(Self {
      id,
      name: resource.name().as_str().into(),
      slug: resource.slug().as_str().into(),
      version: version.number(),
      published_at: version.published_at(),
      body: version.body().clone(),
      used_by: tasks_using(db, id)?,
      assignables: Arc::new(Assignables::load(db)?),
      imported: imported(db, id)?.map(Arc::new),
      team: db.is_team_resource(id)?,
    })
  }

  /// [`Self::load`] from the project database of `repo_root`.
  pub fn load_in(repo_root: &Path, id: ResourceId) -> Result<Self, OpenError> {
    Self::load(&ProjectDb::open(repo_root)?, id)
  }
}

/// Number of the project's tasks that use pipeline `id`.
pub fn tasks_using<S: TaskStore>(store: &S, id: ResourceId) -> Result<usize, S::Error> {
  Ok(
    store
      .tasks()?
      .iter()
      .filter(|task| task.details().pipeline == Some(id))
      .count(),
  )
}
