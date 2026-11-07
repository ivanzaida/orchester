//! What every fill-in checks first: that the project is a team's and that what it names is the team's.

use orchester_core::{
  id::{ResourceId, ServerId},
  resource::ResourceKind,
  run::Answerer,
  store::ResourceStore,
};
use orchester_store::{ProjectDb, ResourceSharing};

use super::FillInError;

/// The team server the project is shared on.
pub(super) fn server_of(db: &ProjectDb) -> Result<ServerId, FillInError> {
  db.sharing()?.map(|link| link.server).ok_or(FillInError::NotTeamProject)
}

/// Fails unless `resource` is a team resource of `kind` in the project.
pub(super) fn require_resource(db: &ProjectDb, resource: ResourceId, kind: ResourceKind) -> Result<(), FillInError> {
  let is_kind = db.resource(resource)?.is_some_and(|found| found.kind() == kind);
  // A resource that does not exist has no mark to read, so only a found one is asked.
  let is_team = is_kind && matches!(db.resource_sharing(resource)?, Some(ResourceSharing::Shared(_)));
  if is_team {
    Ok(())
  } else {
    Err(FillInError::NotTeamResource { resource, kind })
  }
}

/// Fails unless `by` is the person.
pub(super) const fn require_person(by: Answerer) -> Result<(), FillInError> {
  match by {
    Answerer::Person => Ok(()),
    Answerer::Automation => Err(FillInError::PersonOnly),
  }
}
