//! Saving a resource's draft (a pipeline, an instruction) as its editor saves it: an existing draft
//! keeps the version it was started from, a new draft starts from the latest published version. Blocks
//! on SQLite.

use std::path::Path;

use orchester_core::{
  id::ResourceId,
  resource::{Draft, DraftSaved, ResourceBody},
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
