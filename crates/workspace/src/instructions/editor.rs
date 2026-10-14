//! Checked instruction writes return the exact committed candidate, never a later writer's draft. Every
//! screen and tool that saves or publishes an instruction goes through them.
use std::{path::Path, sync::Arc};

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{Draft, DraftSaved, InjectionEvent, Instruction, ResourceBody, Slug, Version},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::{
  data::InstructionEntry,
  publication::{Unavailable, missing},
};
use crate::messages::{self, Message};

/// Why a checked instruction write did not happen.
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
  #[error(transparent)]
  Store(#[from] StoreError),
  /// A variable of the rule has no value at one of its events; nothing was published.
  #[error("{} has no value at {:?}", .0.variable.name(), .0.event)]
  Unavailable(Unavailable),
}

impl WriteError {
  pub fn message(&self) -> Message {
    match self {
      Self::Store(error) => messages::store_error(error),
      Self::Unavailable(Unavailable { variable, event }) => Message::new(match event {
        InjectionEvent::Start => "instructions.errors.variableUnavailableAtStart",
        InjectionEvent::Resume => "instructions.errors.variableUnavailableAtResume",
      })
      .with("variable", format!("{{{{{}}}}}", variable.name())),
    }
  }

  /// A conflicting write: the saved draft must be read again before a retry.
  pub const fn needs_reload(&self) -> bool {
    matches!(self, Self::Store(StoreError::DraftChanged(_)))
  }
}

/// Creates the first draft with its complete validated body in a single store operation.
pub fn create_in(root: &Path, name: Name, body: &Instruction) -> Result<InstructionEntry, WriteError> {
  let mut db = ProjectDb::open(root)?;
  let taken = db
    .resources(Instruction::KIND)?
    .iter()
    .map(|resource| resource.slug().clone())
    .collect::<Vec<_>>();
  let fallback = Slug::new("instruction").expect("the fallback slug is valid");
  let slug = Slug::unique_for(&name, &fallback, &taken);
  let now = Timestamp::now();
  let resource = db.create_resource(slug, name, body, now)?;
  Ok(committed(
    resource.id(),
    (resource.name().as_str().into(), resource.slug().as_str().into()),
    Some(Draft::new(body.clone(), None, now)),
    None,
  ))
}
/// Saves only while the body, latest version and draft timestamp still match what was opened.
pub fn save_in(root: &Path, expected: &InstructionEntry, body: Instruction) -> Result<InstructionEntry, WriteError> {
  let mut db = ProjectDb::open(root)?;
  let now = Timestamp::now();
  let candidate = Draft::new(body, expected.based_on(), now);
  let mut matched_latest = None;
  let outcome = db.update_draft(expected.id, |draft, latest| {
    ensure_same(expected, draft, latest.as_ref())?;
    matched_latest = latest;
    Ok(candidate.clone())
  })?;
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
/// Publishes exactly the saved draft `expected`, if every variable it uses has a value at each of its
/// events; a stale draft is refused by the shared lifecycle.
pub fn publish_in(root: &Path, expected: &InstructionEntry) -> Result<InstructionEntry, WriteError> {
  if let Some(unavailable) = missing(expected.body.text(), expected.body.events()).first() {
    return Err(WriteError::Unavailable(*unavailable));
  }
  let mut db = ProjectDb::open(root)?;
  let version = db.publish_draft_with(expected.id, Timestamp::now(), |draft: Draft<Instruction>, latest| {
    ensure_same(expected, Some(draft.clone()), latest.as_ref())?;
    Ok(draft)
  })?;
  Ok(committed(
    expected.id,
    (expected.name.clone(), expected.slug.clone()),
    None,
    Some(&version),
  ))
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
