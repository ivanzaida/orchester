//! A pipeline is stored only as a published version (D-032): an edit of one is saved by publishing it as the next
//! version, in one transaction, and nothing of it is kept as a draft. A new pipeline is created as its first version
//! ([`ProjectDb::create_published`]).

use orchester_core::{
  id::{ResourceId, VersionId},
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
  /// ([`StoreError::DraftChanged`]) and a body equal to the latest version ([`StoreError::Unchanged`]); nothing is
  /// changed then. In a team project the person must be an admin, and the version waits for the server.
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
    let latest = used_here::<Pipeline>(&transaction, resource)?;
    if latest.as_ref().is_some_and(|version| version.body() == body) {
      return Err(StoreError::Unchanged(resource));
    }
    let latest = latest.map(|version| version.number());
    let edit = Draft::new(body.clone(), Some(based_on), now);
    let version = publish_prepared(&transaction, resource, edit, latest, now)?;
    transaction.commit()?;
    Ok(version)
  }

  /// Takes back `version` of a team pipeline, published here and not numbered by the server (refused, met by a
  /// version someone published from the same base, or still waiting to be sent), with the versions of the pipeline
  /// published here after it, and returns the newest one's steps. A pipeline holds no draft (D-032), so nothing is kept
  /// of them here: the caller hands the steps back to the person as an edit not saved yet. A version a run or a task
  /// here uses stays ([`StoreError::VersionInUse`]); nothing is changed then.
  pub fn take_back_pipeline(&mut self, version: VersionId) -> Result<Pipeline, StoreError> {
    let transaction = self.connection.transaction()?;
    // The versions go before their rows in `shared_publishes`, which the delete trigger reads.
    transaction.pragma_update(None, "defer_foreign_keys", true)?;
    let (_, taken) = shared_publishes::unsent::<Pipeline>(&transaction, version)?;
    let body = shared_publishes::newest_body(&taken, version)?;
    shared_publishes::drop_unsent(&transaction, &taken)?;
    transaction.commit()?;
    Ok(body)
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

  /// A pipeline of this computer's own in a team project, for a test: one kept *Not shared* when the project joined,
  /// at v1, waiting for no server. There [`ProjectDb::create_published`] makes a team pipeline, whose v1 waits for the
  /// server. The slug and the name given as text must be valid.
  #[cfg(feature = "test-support")]
  pub fn kept_pipeline_for_test(
    &mut self,
    slug: &str,
    name: &str,
    body: &Pipeline,
    now: Timestamp,
  ) -> Result<(ResourceId, Version<Pipeline>), StoreError> {
    let slug = orchester_core::resource::Slug::new(slug).expect("a test's pipeline slug is valid");
    let name = Name::new(name).expect("a test's pipeline name is valid");
    let transaction = self.connection.transaction()?;
    let resource = super::new_resources::insert::<Pipeline>(&transaction, slug, name, now)?.id();
    super::shared_versions::set_sharing(&transaction, resource, Some(crate::ResourceSharing::NotShared))?;
    let first = Draft::new(body.clone(), None, now);
    let version = publish_prepared(&transaction, resource, first, None, now)?;
    transaction.commit()?;
    Ok((resource, version))
  }
}
