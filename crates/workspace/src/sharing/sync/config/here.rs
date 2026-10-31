//! What the team's versions need of this computer (`docs/design/shared-configuration.md`, decisions 7, 8 and 11): each
//! connection, MCP server and secret a cached version names, with the resources that name it and how it stands here —
//! bound (by name, by role or by the person), an MCP server found here by its name that waits for the person, or
//! missing — and the local connections, MCP servers and secrets the person may bind it to. Sharing › On this computer
//! lists it all; a resource's page says what of it is missing here and a profile's *Runs here use* which connection
//! its runs take.

use std::path::Path;

use orchester_core::{
  connection::{Connection, mcp::McpServer},
  id::{ConnectionId, ResourceId},
  resource::{AgentProfile, Instruction, Pipeline, ResourceBody, ResourceKind, Skill, Tool, VersionNumber},
  secret::{Secret, SecretKind},
  sharing::ConnectionRole,
  store::{ConnectionStore, McpServerStore, ResourceStore, SecretStore},
  time::Timestamp,
};
use orchester_store::{BoundBy, BoundReference, ProjectDb, ReferenceBinding, StoreError};

use super::bindings::binding_target;
use crate::sharing::SharingError;

/// A resource whose cached versions name a reference, with the newest of them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Needer {
  pub resource: ResourceId,
  pub kind: ResourceKind,
  pub name: String,
  pub number: VersionNumber,
  /// Every cached version of it that names the reference, oldest first.
  pub numbers: Vec<VersionNumber>,
  /// The version of it runs use here, whether it names the reference or not; `None` when every version waits for the
  /// person's *Use v<N> here*.
  pub used: Option<VersionNumber>,
}

/// How a reference stands on this computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Standing {
  /// Bound to the local one named `local`, by its name, its role or the person.
  Bound { local: String, by: BoundBy },
  /// An MCP server with no binding, while a local one has its name: binding it widens what agents may do, so it waits
  /// for the person to confirm it.
  Found { local: ConnectionId, name: String },
  /// Nothing here is bound to it: what names it can't run here.
  Missing,
}

/// One reference a cached version names, as this computer has it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HereReference {
  pub binding: ReferenceBinding,
  pub needed_by: Vec<Needer>,
  pub standing: Standing,
}

impl HereReference {
  /// Whether nothing here is bound to it, so what names it can't run here. An MCP server found by its name is not
  /// missing: it waits for the person's *Confirm…* on its own row.
  pub const fn is_missing(&self) -> bool {
    matches!(self.standing, Standing::Missing)
  }

  /// Whether the version of `resource` its runs use here names it; with no such version, the newest that names it.
  pub fn used_by(&self, resource: ResourceId) -> bool {
    self
      .needed_by
      .iter()
      .any(|needer| needer.resource == resource && needer.numbers.contains(&needer.used.unwrap_or(needer.number)))
  }

  /// Whether version `number` of `resource` names it.
  pub fn named_in(&self, resource: ResourceId, number: VersionNumber) -> bool {
    self
      .needed_by
      .iter()
      .any(|needer| needer.resource == resource && needer.numbers.contains(&number))
  }
}

/// What this computer has of a shared project's references, and what it may bind them to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Here {
  /// Connections first, then MCP servers, then secrets, each kind in the order the publisher made them (their ids
  /// are time-ordered), so a row keeps its place when it is bound or goes missing.
  pub references: Vec<HereReference>,
  pub connections: Vec<Connection>,
  pub mcp_servers: Vec<McpServer>,
  pub secrets: Vec<Secret>,
}

impl Here {
  /// Reads it from the project at `root`.
  pub fn read(root: &Path) -> Result<Self, SharingError> {
    let db = ProjectDb::open(root)?;
    let (connections, mcp_servers, secrets) = (db.connections()?, db.mcp_servers()?, db.secrets()?);
    let mut references = db
      .reference_bindings()?
      .into_iter()
      .map(|binding| {
        let needed_by = needers(&db, &binding)?;
        let standing = standing(&binding, (&connections, &mcp_servers, &secrets));
        Ok(HereReference {
          binding,
          needed_by,
          standing,
        })
      })
      .collect::<Result<Vec<_>, SharingError>>()?;
    // The store lists them by reference id; a stable sort by kind keeps that order within each kind.
    references.sort_by_key(|reference| kind_order(reference.binding.reference));
    Ok(Self {
      references,
      connections,
      mcp_servers,
      secrets,
    })
  }

  /// How many references are missing here, each for the person to choose (*3 need you*). An MCP server found by its
  /// name says *Waits for you* on its own row instead.
  pub fn needs_you(&self) -> usize {
    self
      .references
      .iter()
      .filter(|reference| reference.is_missing())
      .count()
  }

  /// The references the version of `resource` its runs use here names that are missing here; what an older version or
  /// one that waits for *Use v<N> here* names doesn't count.
  pub fn missing_for(&self, resource: ResourceId) -> Vec<&HereReference> {
    self
      .references
      .iter()
      .filter(|reference| reference.is_missing() && reference.used_by(resource))
      .collect()
  }

  /// The references version `number` of `resource` names that version `before` does not (any, for none): what it adds.
  pub fn added_in(
    &self,
    resource: ResourceId,
    number: VersionNumber,
    before: Option<VersionNumber>,
  ) -> Vec<&HereReference> {
    self
      .references
      .iter()
      .filter(|reference| {
        reference.named_in(resource, number) && before.is_none_or(|before| !reference.named_in(resource, before))
      })
      .collect()
  }

  /// The connection reference the version of `resource` its runs use here names: the one a profile's runs take here
  /// (*Runs here use*).
  pub fn connection_of(&self, resource: ResourceId) -> Option<&HereReference> {
    self.references.iter().find(|reference| {
      matches!(reference.binding.reference, BoundReference::Connection { .. }) && reference.used_by(resource)
    })
  }

  /// The local connections of `role` (`claude_cli`, `openai_api`, …), which a connection reference of that role may be
  /// bound to.
  pub fn connections_of(&self, role: &str) -> Vec<&Connection> {
    self
      .connections
      .iter()
      .filter(|connection| ConnectionRole::of(connection.kind()).as_str() == role)
      .collect()
  }

  /// The local secrets a secret reference of `role` may be bound to: of its kind, or all of them when its kind is not
  /// known here.
  pub fn secrets_of(&self, role: Option<&str>) -> Vec<&Secret> {
    let kind = role.and_then(|role| role.parse::<SecretKind>().ok());
    self
      .secrets
      .iter()
      .filter(|secret| kind.is_none_or(|kind| secret.kind() == kind))
      .collect()
  }
}

/// The resources whose cached versions name `binding`'s reference, each with the versions that do and the version its
/// runs use here.
fn needers(db: &ProjectDb, binding: &ReferenceBinding) -> Result<Vec<Needer>, StoreError> {
  let mut needers: Vec<Needer> = Vec::new();
  for naming in db.versions_naming(binding_target(binding.reference))? {
    if let Some(kept) = needers.iter_mut().find(|needer| needer.resource == naming.resource) {
      kept.number = kept.number.max(naming.number);
      kept.numbers.push(naming.number);
      continue;
    }
    let Some(resource) = db.resource(naming.resource)? else {
      continue;
    };
    needers.push(Needer {
      resource: naming.resource,
      kind: naming.kind,
      name: resource.name().as_str().to_owned(),
      number: naming.number,
      numbers: vec![naming.number],
      used: used_version(db, naming.resource, naming.kind)?,
    });
  }
  Ok(needers)
}

/// The version of `resource` runs use here: the newest that does not wait for the person's *Use v<N> here*.
fn used_version(db: &ProjectDb, resource: ResourceId, kind: ResourceKind) -> Result<Option<VersionNumber>, StoreError> {
  fn number<B: ResourceBody>(db: &ProjectDb, resource: ResourceId) -> Result<Option<VersionNumber>, StoreError> {
    Ok(db.latest_version::<B>(resource)?.map(|version| version.number()))
  }
  match kind {
    ResourceKind::Pipeline => number::<Pipeline>(db, resource),
    ResourceKind::AgentProfile => number::<AgentProfile>(db, resource),
    ResourceKind::Instruction => number::<Instruction>(db, resource),
    ResourceKind::Tool => number::<Tool>(db, resource),
    ResourceKind::Skill => number::<Skill>(db, resource),
    // Agent testing's kinds are never shared.
    ResourceKind::TestSuite | ResourceKind::Service | ResourceKind::Stack => Ok(None),
  }
}

fn standing(
  binding: &ReferenceBinding,
  (connections, mcp_servers, secrets): (&[Connection], &[McpServer], &[Secret]),
) -> Standing {
  let bound =
    |local: Option<String>, by: BoundBy| local.map_or(Standing::Missing, |local| Standing::Bound { local, by });
  match binding.reference {
    BoundReference::Connection { bound: Some(found), .. } => bound(
      connections
        .iter()
        .find(|connection| connection.id() == found.local)
        .map(|connection| connection.name().as_str().to_owned()),
      found.by,
    ),
    BoundReference::McpServer { bound: Some(found), .. } => bound(
      mcp_servers
        .iter()
        .find(|server| server.id() == found.local)
        .map(|server| server.name().as_str().to_owned()),
      found.by,
    ),
    BoundReference::Secret { bound: Some(found), .. } => bound(
      secrets
        .iter()
        .find(|secret| secret.id() == found.local)
        .map(|secret| secret.name().as_str().to_owned()),
      found.by,
    ),
    BoundReference::McpServer { bound: None, .. } => {
      let name = binding.name.to_lowercase();
      mcp_servers
        .iter()
        .find(|server| server.name().as_str().to_lowercase() == name)
        .map_or(Standing::Missing, |server| Standing::Found {
          local: server.id(),
          name: server.name().as_str().to_owned(),
        })
    }
    BoundReference::Connection { bound: None, .. } | BoundReference::Secret { bound: None, .. } => Standing::Missing,
  }
}

const fn kind_order(reference: BoundReference) -> u8 {
  match reference {
    BoundReference::Connection { .. } => 0,
    BoundReference::McpServer { .. } => 1,
    BoundReference::Secret { .. } => 2,
  }
}

/// When the person last changed `binding`, for *confirmed Oct 27*; `None` for one bound automatically.
pub const fn chosen_at(binding: &ReferenceBinding) -> Option<Timestamp> {
  match binding.reference {
    BoundReference::Connection { bound: Some(bound), .. } | BoundReference::McpServer { bound: Some(bound), .. }
      if matches!(bound.by, BoundBy::Person) =>
    {
      Some(binding.changed_at)
    }
    BoundReference::Secret { bound: Some(bound), .. } if matches!(bound.by, BoundBy::Person) => {
      Some(binding.changed_at)
    }
    _ => None,
  }
}
