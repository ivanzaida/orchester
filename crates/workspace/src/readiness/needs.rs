//! What each MCP server of the team asks of this computer, and the entry every kind of resource is read into.

use orchester_core::{
  id::{ConnectionId, DecisionConnectionId, ResourceId},
  resource::{Resource, ResourceKind, TeamMcpServer, VersionNumber},
  sharing::{McpNeed, Needs},
  store::ResourceStore,
};
use orchester_store::{ProjectDb, StoreError};

use super::{facts::Found, standing::Undecided};

/// A resource and what it needs, before it is checked against what this computer has.
pub(super) struct Entry {
  pub(super) resource: ResourceId,
  pub(super) kind: ResourceKind,
  pub(super) name: String,
  pub(super) number: VersionNumber,
  pub(super) needs: Result<Needs, Undecided>,
  pub(super) off_here: bool,
  /// What looking at this computer for the needs found (a local project's program files and secret values).
  pub(super) found: Vec<Found>,
}

impl Entry {
  pub(super) fn new(resource: &Resource, number: VersionNumber, needs: Result<Needs, Undecided>) -> Self {
    Self {
      resource: resource.id(),
      kind: resource.kind(),
      name: resource.name().as_str().to_owned(),
      number,
      needs,
      off_here: false,
      found: Vec::new(),
    }
  }
}

/// A team MCP server the project offers, with the version it is read from.
pub(super) struct Server {
  pub(super) entry: Entry,
  pub(super) need: McpNeed,
}

/// A profile's entry and the connection it runs on: the pick, or its own connection in a local project.
pub(super) struct Profile {
  pub(super) entry: Entry,
  pub(super) pick: Option<ConnectionId>,
}

/// A team reviewer's entry and the connection it runs on: the pick, of the kind its body names.
pub(super) struct Reviewer {
  pub(super) entry: Entry,
  pub(super) pick: Option<ConnectionId>,
  pub(super) decision_pick: Option<DecisionConnectionId>,
}

pub(super) fn servers(db: &ProjectDb) -> Result<Vec<Server>, StoreError> {
  let mut servers = Vec::new();
  for resource in db.resources(ResourceKind::McpServer)? {
    let Some(version) = db.latest_version::<TeamMcpServer>(resource.id())? else {
      continue;
    };
    let need = McpNeed::of(resource.id(), version.body());
    let needs = Needs::mcp_server(&need);
    servers.push(Server {
      entry: Entry::new(&resource, version.number(), Ok(needs)),
      need,
    });
  }
  Ok(servers)
}
