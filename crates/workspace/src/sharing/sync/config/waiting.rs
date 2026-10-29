//! A teammate's version that widens what this computer's agents may do (`docs/architecture/server.md`, row 35, "A
//! teammate's version that widens"): it is cached, but runs keep the version this computer used before it until the
//! person's *Use v<N> here* ([`use_here`]). Core decides with [`widens`], over the local bodies, which name the same
//! bindings; a version this computer published itself never waits, and one that does not widen applies at once.

use std::path::Path;

use orchester_core::{
  id::ResourceId,
  resource::{AgentProfile, Instruction, Pipeline, ResourceBody, ResourceKind, Tool, Version, VersionNumber},
  run::Answerer,
  sharing::{SharedBody, VersionWidening, widens},
  store::ResourceStore,
};
use orchester_store::{ProjectDb, StoreError};

use crate::sharing::SharingError;

/// The newest version of a shared resource that waits for the person's *Use v<N> here*, with what it adds over the
/// version runs use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Waiting {
  pub number: VersionNumber,
  pub adds: Vec<VersionWidening>,
}

/// The person's *Use v<N> here*: version `number` of `resource`, which waited, is the one runs use from now on, and the
/// versions before it wait no more; `false` when it did not wait. Only a person presses it: it widens what agents may
/// do here, so automation is refused ([`SharingError::PersonOnly`]) and no MCP tool reaches this.
pub fn use_here(root: &Path, resource: ResourceId, number: VersionNumber, by: Answerer) -> Result<bool, SharingError> {
  person_only(by)?;
  Ok(ProjectDb::open(root)?.use_shared_version(resource, number)?)
}

/// The newest version of `resource` that waits here, with what it adds; `None` when none waits.
pub fn waiting(root: &Path, resource: ResourceId) -> Result<Option<Waiting>, SharingError> {
  let db = ProjectDb::open(root)?;
  let Some(number) = db.waiting_version(resource)? else {
    return Ok(None);
  };
  let kind = db
    .resource(resource)?
    .ok_or(StoreError::ResourceNotFound(resource))?
    .kind();
  let adds = match kind {
    ResourceKind::Pipeline => adds::<Pipeline>(&db, resource, number)?,
    ResourceKind::AgentProfile => adds::<AgentProfile>(&db, resource, number)?,
    ResourceKind::Instruction => adds::<Instruction>(&db, resource, number)?,
    ResourceKind::Tool => adds::<Tool>(&db, resource, number)?,
    // Skills are not kept here yet, and agent testing's kinds are never shared.
    ResourceKind::Skill | ResourceKind::TestSuite | ResourceKind::Service | ResourceKind::Stack => Vec::new(),
  };
  Ok(Some(Waiting { number, adds }))
}

/// Fails unless `by` is the person.
pub(super) const fn person_only(by: Answerer) -> Result<(), SharingError> {
  match by {
    Answerer::Person => Ok(()),
    Answerer::Automation => Err(SharingError::PersonOnly),
  }
}

/// Whether version `number` of `resource`, a teammate's with the local body `next`, waits here: it widens what agents
/// may do over the version this computer uses (none before the first). One this computer has already — published
/// here, or cached before — keeps what it was.
pub(super) fn waits<B: ResourceBody + SharedBody>(
  db: &ProjectDb,
  resource: ResourceId,
  number: VersionNumber,
  next: &B,
) -> Result<bool, StoreError> {
  if db.version::<B>(resource, number)?.is_some() {
    return Ok(
      db.cached_version::<B>(resource, number)?
        .is_some_and(|cached| cached.waits_here),
    );
  }
  let used = db.latest_version::<B>(resource)?;
  Ok(!widens(used.as_ref().map(Version::body), next).is_empty())
}

fn adds<B: ResourceBody + SharedBody>(
  db: &ProjectDb,
  resource: ResourceId,
  number: VersionNumber,
) -> Result<Vec<VersionWidening>, StoreError> {
  let Some(next) = db.version::<B>(resource, number)? else {
    return Ok(Vec::new());
  };
  let used = db.latest_version::<B>(resource)?;
  Ok(widens(used.as_ref().map(Version::body), next.body()))
}
