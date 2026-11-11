//! A pipeline is stored only as a published version (D-032): an edit of one is saved by publishing it as the next
//! version, in one transaction, and nothing of it is kept as a draft. A new pipeline is created as its first version
//! ([`ProjectDb::create_published`]).

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{Draft, Pipeline, Version, VersionNumber},
  time::Timestamp,
};
use rusqlite::{TransactionBehavior, params};

use super::{
  ProjectDb,
  resources::{check_kind, publish_prepared},
  shared_publishes,
  used_versions::used_here,
};
use crate::StoreError;

impl ProjectDb {
  /// Publishes `body`, an edit of pipeline `resource` made from its version `based_on`, as the next version, and
  /// renames the pipeline from `names.0` to `names.1` when asked, atomically. A body that cannot be published is
  /// refused, as is an edit made from a version that is no longer the one used here
  /// ([`orchester_core::resource::PublishError::StaleDraft`]) and a rename whose name changed meanwhile
  /// ([`StoreError::DraftChanged`]); nothing is changed then. In a team project the person must be an admin, and the
  /// version waits for the server.
  pub fn publish_pipeline_edit(
    &mut self,
    resource: ResourceId,
    body: &Pipeline,
    based_on: VersionNumber,
    names: Option<(&str, &Name)>,
    now: Timestamp,
  ) -> Result<Version<Pipeline>, StoreError> {
    let transaction = self
      .connection
      .transaction_with_behavior(TransactionBehavior::Immediate)?;
    check_kind::<Pipeline>(&transaction, resource)?;
    shared_publishes::ensure_may_change(&transaction, resource)?;
    if let Some((before, after)) = names {
      let changed = transaction.execute(
        "UPDATE resources SET name = ?1 WHERE id = ?2 AND name = ?3",
        params![after.as_str(), resource.to_string(), before],
      )?;
      if changed != 1 {
        return Err(StoreError::DraftChanged(resource));
      }
    }
    let latest = used_here::<Pipeline>(&transaction, resource)?.map(|version| version.number());
    let edit = Draft::new(body.clone(), Some(based_on), now);
    let version = publish_prepared(&transaction, resource, edit, latest, now)?;
    transaction.commit()?;
    Ok(version)
  }

  /// A published pipeline in one call, for a test: [`ProjectDb::create_published`] with a slug and a name given as
  /// text, which must be valid.
  #[cfg(feature = "test-support")]
  pub fn published_pipeline_for_test(
    &mut self,
    slug: &str,
    name: &str,
    body: &Pipeline,
    now: Timestamp,
  ) -> Result<(ResourceId, Version<Pipeline>), StoreError> {
    let slug = orchester_core::resource::Slug::new(slug).expect("a test's pipeline slug is valid");
    let name = Name::new(name).expect("a test's pipeline name is valid");
    let (resource, version) = self.create_published(slug, name, body, now)?;
    Ok((resource.id(), version))
  }
}
