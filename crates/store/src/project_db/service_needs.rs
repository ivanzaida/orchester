//! The check a service or stack passes to be published ([`orchester_core::service::PublishNeeds`]), against the
//! services published in the same transaction that publishes it.

use orchester_core::{
  id::ResourceId,
  resource::{PublishError, ResourceBody, ResourceKind},
  service::Service,
};
use rusqlite::{Connection, params};

use super::resources::{kind_text, load_resources, used_here};
use crate::StoreError;

/// Refused with [`PublishError::Needs`] when `body`, published as `resource`, would name a service that is not
/// published or close a cycle of needs. Removed services count as not published.
pub(super) fn check<B: ResourceBody>(
  connection: &Connection,
  resource: ResourceId,
  body: &B,
) -> Result<(), StoreError> {
  let Some(needs) = body.publish_needs() else {
    return Ok(());
  };
  let others = load_resources(
    connection,
    "WHERE kind = ?1 AND id != ?2 AND id NOT IN (SELECT resource_id FROM removed_resources)",
    params![kind_text(ResourceKind::Service), resource.to_string()],
  )?;
  let mut published = Vec::with_capacity(others.len());
  for other in others {
    let latest = used_here::<Service>(connection, other.id())?;
    published.extend(latest.map(|version| version.body().clone()));
  }
  needs.check(&published).map_err(PublishError::from)?;
  Ok(())
}
