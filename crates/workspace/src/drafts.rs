//! Saving a resource's draft (a pipeline, an instruction) as its editor saves it: an existing draft
//! keeps the version it was started from, a new draft starts from the latest published version. Blocks
//! on SQLite.

use std::path::Path;

use orchester_core::{
  id::ResourceId,
  resource::{Draft, DraftSaved, ResourceBody, Version},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

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

/// What a draft save is made on, as automation reads it: when the draft was last saved, if there is one, and
/// the latest published version, the two a person's decision on a draft also names.
pub fn draft_revision<B: ResourceBody>(draft: Option<&Draft<B>>, latest: Option<&Version<B>>) -> String {
  let saved = draft.map_or_else(
    || "none".to_owned(),
    |draft| draft.updated_at().unix_millis().to_string(),
  );
  let latest = latest.map_or_else(|| "none".to_owned(), |version| version.number().get().to_string());
  format!("draft:{saved};latest:{latest}")
}

/// [`draft_revision`] of resource `id` as stored now.
pub fn revision_of<B: ResourceBody, S: ResourceStore>(store: &S, id: ResourceId) -> Result<String, S::Error> {
  Ok(draft_revision(
    store.draft::<B>(id)?.as_ref(),
    store.latest_version::<B>(id)?.as_ref(),
  ))
}

/// [`save_draft_in`], with `expected` only while the draft and the latest version are the ones it names
/// ([`draft_revision`]), checked in the transaction that saves ([`StoreError::DraftChanged`] otherwise). Without
/// `expected` the save goes over whatever is stored, a change saved meanwhile included. The draft is stamped
/// later than the one it replaces, so two saves within one millisecond still name revisions of their own.
pub fn save_draft_expecting_in<B: ResourceBody>(
  repo_root: &Path,
  id: ResourceId,
  body: B,
  expected: Option<&str>,
) -> Result<DraftSaved, StoreError> {
  ProjectDb::open(repo_root)?.update_draft(id, |draft, latest| {
    if expected.is_some_and(|expected| draft_revision(draft.as_ref(), latest.as_ref()) != expected) {
      return Err(StoreError::DraftChanged(id));
    }
    let now = Timestamp::now();
    let at = draft.as_ref().map_or(now, |draft| {
      now.max(Timestamp::from_unix_millis(
        draft.updated_at().unix_millis().saturating_add(1),
      ))
    });
    let based_on = draft.map_or_else(|| latest.as_ref().map(Version::number), |draft| draft.based_on());
    Ok(Draft::new(body, based_on, at))
  })
}
