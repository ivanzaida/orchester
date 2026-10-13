//! Checked instruction writes return the exact committed candidate, never a later writer's draft.
use std::{path::Path, sync::Arc};

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{Draft, DraftSaved, Instruction, ResourceBody, Slug, Version},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::data::InstructionEntry;
use crate::messages::{self, Message};

/// Creates the first draft with its complete validated body in a single store operation.
pub fn create_in(root: &Path, name: Name, body: &Instruction) -> Result<InstructionEntry, Message> {
  let mut db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
  let taken = db
    .resources(Instruction::KIND)
    .map_err(|error| messages::store_error(&error))?
    .iter()
    .map(|resource| resource.slug().clone())
    .collect::<Vec<_>>();
  let fallback = Slug::new("instruction").expect("the fallback slug is valid");
  let slug = Slug::unique_for(&name, &fallback, &taken);
  let now = Timestamp::now();
  let resource = db
    .create_resource(slug, name, body, now)
    .map_err(|error| messages::store_error(&error))?;
  Ok(committed(
    resource.id(),
    (resource.name().as_str().into(), resource.slug().as_str().into()),
    Some(Draft::new(body.clone(), None, now)),
    None,
  ))
}
/// Saves only while the body, latest version and draft timestamp still match what was opened.
pub fn save_in(root: &Path, expected: &InstructionEntry, body: Instruction) -> Result<InstructionEntry, Message> {
  let mut db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
  let now = Timestamp::now();
  let candidate = Draft::new(body, expected.based_on(), now);
  let mut matched_latest = None;
  let outcome = db
    .update_draft(expected.id, |draft, latest| {
      ensure_same(expected, draft, latest.as_ref())?;
      matched_latest = latest;
      Ok(candidate.clone())
    })
    .map_err(|error| messages::store_error(&error))?;
  let draft = if outcome == DraftSaved::Discarded {
    None
  } else {
    Some(candidate)
  };
  Ok(committed(
    expected.id,
    (expected.name.clone(), expected.slug.clone()),
    draft,
    matched_latest.as_ref(),
  ))
}
/// Publishes exactly the saved draft; a stale draft is refused by the shared lifecycle.
pub fn publish_in(root: &Path, expected: &InstructionEntry) -> Result<InstructionEntry, Message> {
  let mut db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
  let version = db
    .publish_draft_with(expected.id, Timestamp::now(), |draft: Draft<Instruction>, latest| {
      ensure_same(expected, Some(draft.clone()), latest.as_ref())?;
      Ok(draft)
    })
    .map_err(|error| messages::store_error(&error))?;
  Ok(committed(
    expected.id,
    (expected.name.clone(), expected.slug.clone()),
    None,
    Some(&version),
  ))
}
/// Publishes a saved draft after rechecking required values in the chosen preview.
pub fn publish_for_in(
  root: &Path,
  expected: &InstructionEntry,
  choice: &super::preview::PreviewChoice,
) -> Result<InstructionEntry, Message> {
  super::publication::ensure_in(root, expected, choice)?;
  publish_in(root, expected)
}

/// A conflicting write whose saved draft must be read again before retrying publication.
pub fn needs_reload(message: &Message) -> bool {
  message.key == "errors.draftChanged"
}
fn ensure_same(
  expected: &InstructionEntry,
  draft: Option<Draft<Instruction>>,
  latest: Option<&Version<Instruction>>,
) -> Result<(), StoreError> {
  let names = (expected.name.clone(), expected.slug.clone());
  if InstructionEntry::new(expected.id, names, draft, latest).as_ref() != Some(expected) {
    return Err(StoreError::DraftChanged(expected.id));
  }
  Ok(())
}
fn committed(
  id: ResourceId,
  names: (Arc<str>, Arc<str>),
  draft: Option<Draft<Instruction>>,
  latest: Option<&Version<Instruction>>,
) -> InstructionEntry {
  InstructionEntry::new(id, names, draft, latest)
    .expect("a successful instruction write leaves a draft or a published version")
}
