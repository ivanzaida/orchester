//! A team copy of a pipeline of this computer's own (`docs/architecture/server.md` row 80): an Admin shares one of
//! their own local pipelines for the team as a new team resource, with a new id, whose first version is the local
//! one's newest. It waits for the server like any first version an admin publishes ([`super::shared_publishes`]) and
//! creates the resource there when it is sent. The local pipeline is kept as it is, its slug shared with the copy while
//! the project is the team's; a copy that never reached the server takes a free slug when the project leaves the team
//! ([`super::team_leaving`]).

use orchester_core::{
  id::ResourceId,
  resource::{Draft, Resource, ResourceBody},
  time::Timestamp,
};

use super::{
  ProjectDb,
  resources::{check_kind, load_resources, publish_prepared},
  shared_publishes::ensure_may_create,
  shared_versions::{ResourceSharing, insert_resource, sharing_of},
  sharing,
  used_versions::used_here,
};
use crate::StoreError;

impl ProjectDb {
  /// Copies `local`, a resource of this computer's own with a version published here, as a new team resource of the
  /// same slug and name, its version waiting for the server; the copy. Refused unless the project is shared
  /// ([`StoreError::NotShared`]) and the person may change the team's configuration ([`StoreError::NotAdmin`]); a
  /// resource that is the team's already is [`StoreError::SharedProject`].
  pub fn copy_for_the_team<B: ResourceBody + Clone>(
    &mut self,
    local: ResourceId,
    now: Timestamp,
  ) -> Result<Resource, StoreError> {
    let transaction = self
      .connection
      .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    check_kind::<B>(&transaction, local)?;
    if sharing::link(&transaction)?.is_none() {
      return Err(StoreError::NotShared);
    }
    let kept = load_resources(&transaction, "WHERE id = ?1", [local.to_string()])?
      .pop()
      .ok_or(StoreError::ResourceNotFound(local))?;
    ensure_may_create(&transaction, kept.kind())?;
    if matches!(sharing_of(&transaction, local)?, Some(ResourceSharing::Shared(_))) {
      return Err(StoreError::SharedProject);
    }
    let version = used_here::<B>(&transaction, local)?.ok_or(StoreError::NotPublished(local))?;
    let copy = Resource::new(
      ResourceId::generate(),
      kept.kind(),
      kept.slug().clone(),
      kept.name().clone(),
      now,
    );
    insert_resource(&transaction, &copy)?;
    publish_prepared(
      &transaction,
      copy.id(),
      Draft::new(version.body().clone(), None, now),
      None,
      now,
    )?;
    transaction.commit()?;
    Ok(copy)
  }
}
