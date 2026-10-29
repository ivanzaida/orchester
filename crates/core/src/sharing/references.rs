//! What a shared version names on each computer (`docs/architecture/server.md`, "Shared configuration"). A body refers
//! to connections, MCP servers and secrets by the publisher's ids, and a published version carries a [`Reference`] per
//! such id; each computer binds a reference to one of its own entities ([`Binding`]). The walks here go from a body to
//! the references it names ([`named`]), through the bindings to the local body runs pin ([`resolve`]), and back to
//! references when this computer publishes ([`for_publishing`]). They are `import`'s reference walk: an id is found
//! and replaced as a JSON string wherever the body holds it.

use std::collections::{HashMap, HashSet};

use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::{
  connection::{AgentProvider, Connection, ConnectionKind, mcp::McpServer},
  id::{ConnectionId, SecretId},
  import::{
    Entity,
    references::{self, Remap},
  },
  secret::{Secret, SecretKind},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ReferenceKind {
  Connection,
  McpServer,
  Secret,
}

impl ReferenceKind {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Connection => "connection",
      Self::McpServer => "mcp_server",
      Self::Secret => "secret",
    }
  }

  const fn of(entity: Entity) -> Option<Self> {
    match entity {
      Entity::Connection(_) => Some(Self::Connection),
      Entity::McpServer(_) => Some(Self::McpServer),
      Entity::Secret(_) => Some(Self::Secret),
      Entity::Resource(_) => None,
    }
  }
}

/// What a connection is for, without any of its settings: the agent provider it runs, or GitHub. A reference to a
/// connection binds to a local connection of the same role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConnectionRole {
  Agent(AgentProvider),
  GitHub,
}

impl ConnectionRole {
  const ALL: [Self; 5] = [
    Self::Agent(AgentProvider::CodexCli),
    Self::Agent(AgentProvider::ClaudeCodeCli),
    Self::Agent(AgentProvider::AnthropicApi),
    Self::Agent(AgentProvider::OpenAiApi),
    Self::GitHub,
  ];

  pub const fn of(kind: &ConnectionKind) -> Self {
    match kind {
      ConnectionKind::Agent(provider) => Self::Agent(*provider),
      ConnectionKind::GitHub { .. } => Self::GitHub,
    }
  }

  /// The role as a reference carries it: the connection's kind as the project database stores it.
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Agent(AgentProvider::CodexCli) => "codex_cli",
      Self::Agent(AgentProvider::ClaudeCodeCli) => "claude_code_cli",
      Self::Agent(AgentProvider::AnthropicApi) => "anthropic_api",
      Self::Agent(AgentProvider::OpenAiApi) => "openai_api",
      Self::GitHub => "github",
    }
  }

  /// The role a reference carries as [`Self::as_str`] writes it; `None` for one this version does not know.
  pub fn parse(role: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|known| known.as_str() == role)
  }
}

/// One connection, MCP server or secret a version names, by its id on the publisher's computer, with what each
/// computer binds it by: its kind, its name and its role. Nothing else about the entity is in it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Reference {
  Connection {
    id: ConnectionId,
    name: String,
    role: ConnectionRole,
  },
  McpServer {
    id: ConnectionId,
    name: String,
  },
  Secret {
    id: SecretId,
    name: String,
    role: SecretKind,
  },
}

impl Reference {
  pub fn connection(connection: &Connection) -> Self {
    Self::Connection {
      id: connection.id(),
      name: connection.name().as_str().to_owned(),
      role: ConnectionRole::of(connection.kind()),
    }
  }

  pub fn mcp_server(server: &McpServer) -> Self {
    Self::McpServer {
      id: server.id(),
      name: server.name().as_str().to_owned(),
    }
  }

  pub fn secret(secret: &Secret) -> Self {
    Self::Secret {
      id: secret.id(),
      name: secret.name().as_str().to_owned(),
      role: secret.kind(),
    }
  }

  pub const fn kind(&self) -> ReferenceKind {
    match self {
      Self::Connection { .. } => ReferenceKind::Connection,
      Self::McpServer { .. } => ReferenceKind::McpServer,
      Self::Secret { .. } => ReferenceKind::Secret,
    }
  }

  pub fn name(&self) -> &str {
    match self {
      Self::Connection { name, .. } | Self::McpServer { name, .. } | Self::Secret { name, .. } => name,
    }
  }

  /// The connection's kind or the secret's; an MCP server has no role.
  pub const fn role(&self) -> Option<&'static str> {
    match self {
      Self::Connection { role, .. } => Some(role.as_str()),
      Self::McpServer { .. } => None,
      Self::Secret { role, .. } => Some(role.as_str()),
    }
  }

  /// The entity the reference names on the publisher's computer.
  pub const fn entity(&self) -> Entity {
    match self {
      Self::Connection { id, .. } => Entity::Connection(*id),
      Self::McpServer { id, .. } => Entity::McpServer(*id),
      Self::Secret { id, .. } => Entity::Secret(*id),
    }
  }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("a {} reference is bound only to a local entity of that kind", .0.as_str())]
pub struct BindingKindError(ReferenceKind);

/// The local connection, MCP server or secret a reference means on this computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
  reference: Reference,
  local: Entity,
}

impl Binding {
  /// `local` must be of the reference's kind.
  pub fn new(reference: Reference, local: Entity) -> Result<Self, BindingKindError> {
    if ReferenceKind::of(local) != Some(reference.kind()) {
      return Err(BindingKindError(reference.kind()));
    }
    Ok(Self { reference, local })
  }

  pub const fn reference(&self) -> &Reference {
    &self.reference
  }

  pub const fn local(&self) -> Entity {
    self.local
  }
}

/// The connections, MCP servers and secrets of one computer, whose ids a body written there holds.
#[derive(Clone, Copy, Debug)]
pub struct LocalEntities<'a> {
  pub connections: &'a [Connection],
  pub mcp_servers: &'a [McpServer],
  pub secrets: &'a [Secret],
}

impl LocalEntities<'_> {
  fn reference(&self, uuid: Uuid) -> Option<Reference> {
    let (connection, secret) = (ConnectionId::from_uuid(uuid), SecretId::from_uuid(uuid));
    let connections = self.connections.iter().find(|entry| entry.id() == connection);
    let servers = || self.mcp_servers.iter().find(|entry| entry.id() == connection);
    let secrets = || self.secrets.iter().find(|entry| entry.id() == secret);
    connections
      .map(Reference::connection)
      .or_else(|| servers().map(Reference::mcp_server))
      .or_else(|| secrets().map(Reference::secret))
  }
}

/// The references `body` names: each connection, MCP server and secret of `entities` whose id is a value of the body,
/// in the order the body first names them. Other ids, such as shared resources', are not references.
pub fn named(body: &Value, entities: LocalEntities<'_>) -> Vec<Reference> {
  references::values(body)
    .into_iter()
    .filter_map(|uuid| entities.reference(uuid))
    .collect()
}

/// `body` with the id of every reference `bindings` bind replaced by the local id, the body runs on this computer pin.
/// A reference without a binding keeps the publisher's id, which matches nothing here, so preflight reports it missing.
pub fn resolve(body: Value, bindings: &[Binding]) -> Value {
  rewrite(
    body,
    bindings
      .iter()
      .map(|binding| (binding.reference.entity().uuid(), binding.local.uuid())),
  )
}

/// A version to publish from this computer: its body in the publisher's ids, and a reference for each it names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Published {
  pub body: Value,
  pub references: Vec<Reference>,
}

/// `body`, written with this computer's ids, turned back into a version to publish. A local id bound to a reference
/// becomes that reference's id and carries it (the first binding, when several are bound to one entity); an id of a
/// reference of `base`, the version the body was edited from, that has no binding here keeps that reference; any
/// other connection, MCP server or secret of `entities` becomes a new reference by its own id.
pub fn for_publishing(body: Value, entities: LocalEntities<'_>, base: &[Reference], bindings: &[Binding]) -> Published {
  let mut bound = HashMap::new();
  for binding in bindings {
    bound.entry(binding.local.uuid()).or_insert(&binding.reference);
  }
  let mut seen = HashSet::new();
  let references = references::values(&body)
    .into_iter()
    .filter_map(|uuid| {
      bound
        .get(&uuid)
        .map(|reference| (*reference).clone())
        .or_else(|| base.iter().find(|reference| reference.entity().uuid() == uuid).cloned())
        .or_else(|| entities.reference(uuid))
    })
    .filter(|reference| seen.insert(reference.entity()))
    .collect();
  let body = rewrite(
    body,
    bound
      .iter()
      .map(|(local, reference)| (*local, reference.entity().uuid())),
  );
  Published { body, references }
}

/// `body` with each id of `ids` replaced by the id it maps to. Keys stay as they are: a map keyed by ids holds shared
/// resources, which keep their ids on every computer.
fn rewrite(body: Value, ids: impl Iterator<Item = (Uuid, Uuid)>) -> Value {
  Remap {
    values: ids.collect(),
    keys: HashMap::new(),
    known: HashSet::new(),
  }
  .rewrite(body)
}
