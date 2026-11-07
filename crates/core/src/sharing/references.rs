//! The local connections, MCP servers and secrets a body of this computer names (`docs/architecture/server.md`,
//! "Shared configuration"). A team resource never carries them (`docs/design/team-configuration.md`: it refers to
//! other team resources by server id, to secrets by name and to a CLI kind, never to an id from a computer), so a body
//! that [`named`] finds any in is refused as a team body. Versions cached before the team configuration model still
//! hold the publisher's ids with a [`Binding`] to this computer's entities each; [`resolve`] writes those bindings into
//! such a body. The walks are `import`'s reference walk: an id is found and replaced as a JSON string wherever the body
//! holds it.

use std::collections::{HashMap, HashSet};

use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use super::ConnectionRole;
use crate::{
  connection::{Connection, mcp::McpServer},
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
