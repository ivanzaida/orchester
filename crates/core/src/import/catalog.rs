//! What a project holds that an import copies from or maps to: its resources with their latest
//! published version and draft, its connections, MCP servers and active secrets.

use serde_json::Value;

use super::{Entity, references};
use crate::{
  connection::{Connection, mcp::McpServer},
  id::{ConnectionId, ResourceId, SecretId},
  project::Project,
  resource::{AgentProfile, Instruction, Pipeline, Resource, ResourceBody, ResourceKind, Tool, VersionNumber},
  secret::Secret,
  store::{ConnectionStore, McpServerStore, ResourceStore, SecretStore},
};

/// The body of a resource of any kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Content {
  AgentProfile(AgentProfile),
  Instruction(Instruction),
  Pipeline(Pipeline),
  Tool(Tool),
}

impl Content {
  pub const fn kind(&self) -> ResourceKind {
    match self {
      Self::AgentProfile(_) => ResourceKind::AgentProfile,
      Self::Instruction(_) => ResourceKind::Instruction,
      Self::Pipeline(_) => ResourceKind::Pipeline,
      Self::Tool(_) => ResourceKind::Tool,
    }
  }

  pub(super) fn to_value(&self) -> Value {
    let value = match self {
      Self::AgentProfile(body) => serde_json::to_value(body),
      Self::Instruction(body) => serde_json::to_value(body),
      Self::Pipeline(body) => serde_json::to_value(body),
      Self::Tool(body) => serde_json::to_value(body),
    };
    value.expect("resource bodies are plain data that serializes to JSON")
  }

  /// A body of this content's kind from JSON, through the domain constructors: a value that breaks
  /// a rule of the kind is an error.
  pub(super) fn with_value(&self, value: Value) -> Result<Self, serde_json::Error> {
    Ok(match self {
      Self::AgentProfile(_) => Self::AgentProfile(serde_json::from_value(value)?),
      Self::Instruction(_) => Self::Instruction(serde_json::from_value(value)?),
      Self::Pipeline(_) => Self::Pipeline(serde_json::from_value(value)?),
      Self::Tool(_) => Self::Tool(serde_json::from_value(value)?),
    })
  }
}

/// Which content of a resource a copy takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pick {
  /// The latest published version, numbered.
  Published(VersionNumber),
  /// The draft: unpublished changes, or a resource never published.
  Draft,
}

/// A resource with the content an import can copy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogResource {
  resource: Resource,
  published: Option<(VersionNumber, Content)>,
  draft: Option<Content>,
}

impl CatalogResource {
  /// `None` for a resource with neither a published version nor a draft: it has nothing to copy.
  pub fn new(resource: Resource, published: Option<(VersionNumber, Content)>, draft: Option<Content>) -> Option<Self> {
    (published.is_some() || draft.is_some()).then_some(Self {
      resource,
      published,
      draft,
    })
  }

  pub const fn resource(&self) -> &Resource {
    &self.resource
  }

  /// The number of the latest published version, if there is one.
  pub fn published(&self) -> Option<VersionNumber> {
    self.published.as_ref().map(|(number, _)| *number)
  }

  pub const fn has_draft(&self) -> bool {
    self.draft.is_some()
  }

  /// The latest published version when there is one, otherwise the draft.
  pub fn default_pick(&self) -> Pick {
    self.published().map_or(Pick::Draft, Pick::Published)
  }

  /// Both picks when the resource has a published version and a draft, else the one it has.
  pub fn picks(&self) -> Vec<Pick> {
    self
      .published()
      .map(Pick::Published)
      .into_iter()
      .chain(self.draft.is_some().then_some(Pick::Draft))
      .collect()
  }

  /// The content of `pick`; `None` when the resource does not have it.
  pub fn content(&self, pick: Pick) -> Option<&Content> {
    match (pick, &self.published, &self.draft) {
      (Pick::Published(wanted), Some((number, content)), _) if wanted == *number => Some(content),
      (Pick::Draft, _, Some(content)) => Some(content),
      _ => None,
    }
  }

  /// `pick` when the resource has it, otherwise its default pick.
  pub(super) fn available(&self, pick: Pick) -> Pick {
    if self.content(pick).is_some() {
      pick
    } else {
      self.default_pick()
    }
  }
}

/// A project's resources, connections, MCP servers and active secrets, as an import reads them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Catalog {
  project: Project,
  resources: Vec<CatalogResource>,
  connections: Vec<Connection>,
  mcp_servers: Vec<McpServer>,
  secrets: Vec<Secret>,
}

impl Catalog {
  pub const fn new(
    project: Project,
    resources: Vec<CatalogResource>,
    connections: Vec<Connection>,
    mcp_servers: Vec<McpServer>,
    secrets: Vec<Secret>,
  ) -> Self {
    Self {
      project,
      resources,
      connections,
      mcp_servers,
      secrets,
    }
  }

  /// Reads `project`'s catalog from its `store`. Only `&S` is needed: reading changes nothing.
  pub fn read<S, E>(project: Project, store: &S) -> Result<Self, E>
  where
    S: ResourceStore<Error = E> + ConnectionStore<Error = E> + McpServerStore<Error = E> + SecretStore<Error = E>,
  {
    // In the order libraries and the import list them. Skills are not copied between projects:
    // their reference files are blobs of their own project.
    let mut resources = read_kind(store, Content::Pipeline)?;
    resources.extend(read_kind(store, Content::AgentProfile)?);
    resources.extend(read_kind(store, Content::Instruction)?);
    resources.extend(read_kind(store, Content::Tool)?);
    Ok(Self::new(
      project,
      resources,
      store.connections()?,
      store.mcp_servers()?,
      store.secrets()?,
    ))
  }

  pub const fn project(&self) -> &Project {
    &self.project
  }

  /// Pipelines, agent profiles, instructions, then tools; each kind by slug.
  pub fn resources(&self) -> &[CatalogResource] {
    &self.resources
  }

  pub fn resource(&self, id: ResourceId) -> Option<&CatalogResource> {
    self.resources.iter().find(|entry| entry.resource.id() == id)
  }

  pub fn connections(&self) -> &[Connection] {
    &self.connections
  }

  pub fn connection(&self, id: ConnectionId) -> Option<&Connection> {
    self.connections.iter().find(|connection| connection.id() == id)
  }

  pub fn mcp_servers(&self) -> &[McpServer] {
    &self.mcp_servers
  }

  pub fn mcp_server(&self, id: ConnectionId) -> Option<&McpServer> {
    self.mcp_servers.iter().find(|server| server.id() == id)
  }

  /// Secrets that are not archived.
  pub fn secrets(&self) -> &[Secret] {
    &self.secrets
  }

  pub fn secret(&self, id: SecretId) -> Option<&Secret> {
    self.secrets.iter().find(|secret| secret.id() == id)
  }

  /// The resources whose default content ([`CatalogResource::default_pick`]) refers to `entity`,
  /// e.g. the pipelines that run a profile.
  pub fn users(&self, entity: Entity) -> impl Iterator<Item = &CatalogResource> {
    let wanted = entity.uuid();
    self.resources.iter().filter(move |resource| {
      resource
        .content(resource.default_pick())
        .is_some_and(|content| references::values(&content.to_value()).contains(&wanted))
    })
  }

  /// Whether `entity` is in the catalog.
  pub fn has(&self, entity: Entity) -> bool {
    match entity {
      Entity::Resource(id) => self.resource(id).is_some(),
      Entity::Connection(id) => self.connection(id).is_some(),
      Entity::McpServer(id) => self.mcp_server(id).is_some(),
      Entity::Secret(id) => self.secret(id).is_some(),
    }
  }

  /// The name of `entity`; `None` when the catalog does not have it.
  pub fn name(&self, entity: Entity) -> Option<&str> {
    match entity {
      Entity::Resource(id) => self.resource(id).map(|entry| entry.resource().name().as_str()),
      Entity::Connection(id) => self.connection(id).map(|connection| connection.name().as_str()),
      Entity::McpServer(id) => self.mcp_server(id).map(|server| server.name().as_str()),
      Entity::Secret(id) => self.secret(id).map(|secret| secret.name().as_str()),
    }
  }

  /// The entity whose id is `uuid`, if the catalog has one.
  pub(super) fn entity(&self, uuid: uuid::Uuid) -> Option<Entity> {
    [
      Entity::Resource(ResourceId::from_uuid(uuid)),
      Entity::Connection(ConnectionId::from_uuid(uuid)),
      Entity::McpServer(ConnectionId::from_uuid(uuid)),
      Entity::Secret(SecretId::from_uuid(uuid)),
    ]
    .into_iter()
    .find(|entity| self.has(*entity))
  }
}

fn read_kind<B, S, E>(store: &S, wrap: fn(B) -> Content) -> Result<Vec<CatalogResource>, E>
where
  B: ResourceBody + Clone,
  S: ResourceStore<Error = E>,
{
  let mut entries = Vec::new();
  for resource in store.resources(B::KIND)? {
    let id = resource.id();
    let published = store
      .latest_version::<B>(id)?
      .map(|version| (version.number(), wrap(version.body().clone())));
    let draft = store.draft::<B>(id)?.map(|draft| wrap(draft.body().clone()));
    entries.extend(CatalogResource::new(resource, published, draft));
  }
  Ok(entries)
}
