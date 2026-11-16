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

/// Creates the instruction as its first version, v1 with no draft, if every variable it uses has a value at each of its
/// events. In a team project the version waits for the server (`Servers::send_created`).
pub fn create_in(root: &Path, name: Name, body: &Instruction) -> Result<InstructionEntry, WriteError> {
  if let Some(unavailable) = missing(body.text(), body.events()).first() {
    return Err(WriteError::Unavailable(*unavailable));
  }
  let mut db = ProjectDb::open(root)?;
  let taken = db.slugs(Instruction::KIND)?;
  let fallback = Slug::new("instruction").expect("the fallback slug is valid");
  let slug = Slug::unique_for(&name, &fallback, &taken);
  let (resource, version) = db.create_published(slug, name, body, Timestamp::now())?;
  Ok(committed(
    resource.id(),
    (resource.name().as_str().into(), resource.slug().as_str().into()),
    None,
    Some(&version),
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
/// Records the draft `saved` just wrote as automation's (ORC-30): an MCP tool's save, or the window's under synthetic
/// input. The Instructions page marks it until a person saves it again or it is published.
pub fn mark_automation_in(root: &Path, saved: &InstructionEntry) -> Result<(), WriteError> {
  if let Some(at) = saved.draft_saved_at {
    ProjectDb::open(root)?.mark_automation_draft(saved.id, at)?;
  }
  Ok(())
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

/// Publishes `body` as the next version of the instruction `expected` at once, as the Agents hub's Save does: saved as
/// its draft over exactly `expected`, then published, if every variable it uses has a value at each of its events. All
/// or nothing: the draft the save made is thrown away when the publication is refused; a draft `expected` already had
/// stays, holding `body`.
pub fn save_and_publish_in(
  root: &Path,
  expected: &InstructionEntry,
  body: Instruction,
) -> Result<InstructionEntry, WriteError> {
  if let Some(unavailable) = missing(body.text(), body.events()).first() {
    return Err(WriteError::Unavailable(*unavailable));
  }
  let drafted = save_in(root, expected, body)?;
  let published = publish_in(root, &drafted);
  if published.is_err() && !expected.state.has_draft() {
    // The refusal is what the person is told; a draft that could not be thrown away is listed, and its panel shows it.
    if let Err(error) = ProjectDb::open(root).and_then(|mut db| db.discard_draft_matching(drafted.id, &drafted.body)) {
      tracing::error!(%error, "cannot discard the draft of an instruction whose publication was refused");
    }
  }
  published
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
