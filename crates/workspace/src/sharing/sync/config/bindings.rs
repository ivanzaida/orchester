//! This computer's binding of each reference a shared version names (`docs/architecture/server.md`, "What a version
//! names on each computer"): made the first time a version names the reference and kept for every later one. A
//! connection binds to the local connection of its role with its name (without regard to case), else to the only
//! local connection of its role; an MCP server or a secret stays unbound until a person binds it ([`super::bind`]),
//! since binding one widens what agents may do or what a tool sends.

use orchester_api::{Reference as SharedReference, ReferenceTarget as SharedTarget};
use orchester_core::{
  connection::Connection,
  id::ConnectionId,
  import::Entity,
  secret::SecretKind,
  sharing::{Binding, ConnectionRole, Reference},
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::{Bound, BoundBy, BoundReference, ProjectDb, ReferenceBinding, ReferenceTarget, StoreError};

/// The bound references this computer has, after giving each of `references` that has no binding yet its first one.
pub(super) fn bind_new(db: &mut ProjectDb, references: &[SharedReference]) -> Result<Vec<Binding>, StoreError> {
  let mut kept = db.reference_bindings()?;
  for reference in references {
    let Some(unbound) = unbound(reference.target) else {
      continue;
    };
    let target = binding_target(unbound);
    if kept.iter().any(|binding| binding_target(binding.reference) == target) {
      continue;
    }
    let first = match unbound {
      BoundReference::Connection { reference: id, .. } => BoundReference::Connection {
        reference: id,
        bound: by_name_or_role(&db.connections()?, &reference.name, reference.role.as_deref()),
      },
      person_only => person_only,
    };
    let binding = ReferenceBinding {
      reference: first,
      name: reference.name.clone(),
      role: reference.role.clone(),
      changed_at: Timestamp::now(),
    };
    db.set_reference_binding(&binding)?;
    kept.push(binding);
  }
  Ok(bound(&kept))
}

/// The bindings of `kept` that bind their reference, as core resolves bodies with them. A reference whose role this
/// version does not know stays unresolved: core names a connection and a secret by their role.
pub(super) fn bound(kept: &[ReferenceBinding]) -> Vec<Binding> {
  kept.iter().filter_map(core_binding).collect()
}

pub(super) const fn binding_target(reference: BoundReference) -> ReferenceTarget {
  match reference {
    BoundReference::Connection { reference, .. } => ReferenceTarget::Connection(reference),
    BoundReference::McpServer { reference, .. } => ReferenceTarget::McpServer(reference),
    BoundReference::Secret { reference, .. } => ReferenceTarget::Secret(reference),
  }
}

/// `target` not bound to anything yet; a shared tool is no reference this computer binds.
const fn unbound(target: SharedTarget) -> Option<BoundReference> {
  match target {
    SharedTarget::Connection { id } => Some(BoundReference::Connection {
      reference: id,
      bound: None,
    }),
    SharedTarget::McpServer { id } => Some(BoundReference::McpServer {
      reference: id,
      bound: None,
    }),
    SharedTarget::Secret { id } => Some(BoundReference::Secret {
      reference: id,
      bound: None,
    }),
    SharedTarget::Tool { .. } => None,
  }
}

/// The local connection of `role` named `name`, without regard to case; else the only local connection of `role`.
fn by_name_or_role(connections: &[Connection], name: &str, role: Option<&str>) -> Option<Bound<ConnectionId>> {
  let role = role?;
  let of_role: Vec<&Connection> = connections
    .iter()
    .filter(|connection| ConnectionRole::of(connection.kind()).as_str() == role)
    .collect();
  let name = name.to_lowercase();
  if let Some(named) = of_role
    .iter()
    .find(|connection| connection.name().as_str().to_lowercase() == name)
  {
    return Some(Bound {
      local: named.id(),
      by: BoundBy::Name,
    });
  }
  match of_role[..] {
    [only] => Some(Bound {
      local: only.id(),
      by: BoundBy::Role,
    }),
    _ => None,
  }
}

fn core_binding(kept: &ReferenceBinding) -> Option<Binding> {
  let name = kept.name.clone();
  let role = kept.role.as_deref();
  let (reference, local) = match kept.reference {
    BoundReference::Connection {
      reference,
      bound: Some(bound),
    } => (
      Reference::Connection {
        id: reference,
        name,
        role: ConnectionRole::parse(role?)?,
      },
      Entity::Connection(bound.local),
    ),
    BoundReference::McpServer {
      reference,
      bound: Some(bound),
    } => (
      Reference::McpServer { id: reference, name },
      Entity::McpServer(bound.local),
    ),
    BoundReference::Secret {
      reference,
      bound: Some(bound),
    } => (
      Reference::Secret {
        id: reference,
        name,
        role: role?.parse::<SecretKind>().ok()?,
      },
      Entity::Secret(bound.local),
    ),
    BoundReference::Connection { bound: None, .. }
    | BoundReference::McpServer { bound: None, .. }
    | BoundReference::Secret { bound: None, .. } => return None,
  };
  Some(Binding::new(reference, local).expect("each kind of binding is to a local entity of its kind"))
}
