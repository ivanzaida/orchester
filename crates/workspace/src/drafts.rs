//! Saving a resource's draft (a pipeline, an instruction) as its editor saves it: an existing draft
//! keeps the version it was started from, a new draft starts from the latest published version. Blocks
//! on SQLite.

use std::path::Path;

use orchester_core::{
  id::ResourceId,
  resource::{Draft, DraftSaved, ResourceBody, Version, VersionNumber},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use sha2::{Digest, Sha256};

/// Saves `body` as the draft of resource `id`; [`DraftSaved::Discarded`] when it repeats the version it
/// is based on, so no draft remains.
pub fn save_draft<B: ResourceBody, S: ResourceStore>(
  store: &mut S,
  id: ResourceId,
  body: B,
  now: Timestamp,
) -> Result<DraftSaved, S::Error> {
  let based_on = match store.draft::<B>(id)? {
    Some(draft) => draft.based_on(),
    None => store.latest_version::<B>(id)?.map(|version| version.number()),
  };
  store.save_draft(id, &Draft::new(body, based_on, now))
}

/// [`save_draft`] in the project database of `repo_root`, now.
pub fn save_draft_in<B: ResourceBody>(repo_root: &Path, id: ResourceId, body: B) -> Result<DraftSaved, StoreError> {
  save_draft(&mut ProjectDb::open(repo_root)?, id, body, Timestamp::now())
}

/// What a draft save is made on, as automation reads it: the draft, if there is one, by when it was last saved,
/// the version it was started from and a digest of its body, and the latest published version. Two drafts that
/// name one revision hold the same body on the same versions, so a save made on it loses nothing.
pub fn draft_revision<B: ResourceBody>(
  draft: Option<&Draft<B>>,
  latest: Option<&Version<B>>,
) -> Result<String, StoreError> {
  let number =
    |number: Option<VersionNumber>| number.map_or_else(|| "none".to_owned(), |number| number.get().to_string());
  let latest = number(latest.map(Version::number));
  let Some(draft) = draft else {
    return Ok(format!("draft:none;latest:{latest}"));
  };
  let body = serde_json::to_vec(draft.body()).map_err(|error| StoreError::Corrupt {
    what: "draft body",
    detail: error.to_string(),
  })?;
  let digest: String = Sha256::digest(&body)[..8]
    .iter()
    .map(|byte| format!("{byte:02x}"))
    .collect();
  Ok(format!(
    "draft:{}@{}#{digest};latest:{latest}",
    draft.updated_at().unix_millis(),
    number(draft.based_on()),
  ))
}

/// [`draft_revision`] of resource `id` as stored now.
pub fn revision_of<B: ResourceBody>(db: &ProjectDb, id: ResourceId) -> Result<String, StoreError> {
  draft_revision(db.draft::<B>(id)?.as_ref(), db.latest_version::<B>(id)?.as_ref())
}

/// [`save_draft_in`], with `expected` only while the draft and the latest version are the ones it names
/// ([`draft_revision`]), checked in the transaction that saves ([`StoreError::DraftChanged`] otherwise). Without
/// `expected` the save goes over whatever is stored, a change saved meanwhile included.
pub fn save_draft_expecting_in<B: ResourceBody>(
  repo_root: &Path,
  id: ResourceId,
  body: B,
  expected: Option<&str>,
) -> Result<DraftSaved, StoreError> {
  ProjectDb::open(repo_root)?.update_draft(id, |draft, latest| {
    if let Some(expected) = expected
      && draft_revision(draft.as_ref(), latest.as_ref())? != expected
    {
      return Err(StoreError::DraftChanged(id));
    }
    let based_on = draft.map_or_else(|| latest.as_ref().map(Version::number), |draft| draft.based_on());
    Ok(Draft::new(body, based_on, Timestamp::now()))
  })
}
