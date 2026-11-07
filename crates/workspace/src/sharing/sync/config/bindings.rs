//! This computer's binding of each reference a version cached before the team configuration model names
//! (`docs/architecture/server.md`, "What a version names on each computer"): kept for those versions until the
//! bindings are removed. A team version names no id of a computer, so nothing makes a new one.

use orchester_core::{
  connection::Connection,
  import::Entity,
  secret::SecretKind,
  sharing::{Binding, ConnectionRole, Reference},
};
use orchester_store::{BoundReference, ReferenceBinding, ReferenceTarget};

/// The bindings of `kept` that bind their reference, as core resolves bodies with them. A reference whose role this
/// version does not know stays unresolved: core names a connection and a secret by their role.
pub(super) fn bound(kept: &[ReferenceBinding], connections: &[Connection]) -> Vec<Binding> {
  kept
    .iter()
    .filter_map(|kept| core_binding(&current(kept.clone(), connections)))
    .collect()
}

pub(super) const fn binding_target(reference: BoundReference) -> ReferenceTarget {
  match reference {
    BoundReference::Connection { reference, .. } => ReferenceTarget::Connection(reference),
    BoundReference::McpServer { reference, .. } => ReferenceTarget::McpServer(reference),
    BoundReference::Secret { reference, .. } => ReferenceTarget::Secret(reference),
  }
}

/// A stale connection binding is unbound here; the person's other choices stay as kept.
pub(super) fn current(mut kept: ReferenceBinding, connections: &[Connection]) -> ReferenceBinding {
  if let BoundReference::Connection {
    reference,
    bound: Some(bound),
  } = kept.reference
    && !connections.iter().any(|connection| {
      connection.id() == bound.local && Some(ConnectionRole::of(connection.kind()).as_str()) == kept.role.as_deref()
    })
  {
    kept.reference = BoundReference::Connection { reference, bound: None };
  }
  kept
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
