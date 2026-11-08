//! The local connections, MCP servers and secrets a body of this computer names (`docs/architecture/server.md`,
//! "Shared configuration"). A team resource never carries them (`docs/design/team-configuration.md`: it refers to
//! other team resources by server id, to secrets by name and to a CLI kind, never to an id from a computer), so a body
//! that [`named`] finds any in is refused as a team body. The walk is `import`'s reference walk: an id is found as a
//! JSON string wherever the body holds it.

use serde_json::Value;
use uuid::Uuid;

use super::ConnectionRole;
use crate::{
  connection::{Connection, mcp::McpServer},
  id::{ConnectionId, SecretId},
  import::{Entity, references},
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
