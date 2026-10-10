//! Editor writes: never overwrite another editor's draft or publish a body other than the one reviewed.

use std::path::Path;

use orchester_core::{
  id::ResourceId,
  resource::{Draft, Pipeline, VersionNumber},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::{
  data::{OpenError, OpenedPipeline},
  problems::publish_error,
  widening::{draft_reasons, publish_reasons},
};
use crate::messages::Message;

/// Exact reasons the person approved; an empty list permits changes that need no approval.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Allowed(pub Vec<Message>);

/// Reads and writes report their failure without losing the current editor.
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
  #[error(transparent)]
  Open(#[from] OpenError),
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error("the pipeline needs a person's decision")]
  PersonOnly,
  #[error("a step's executor or narrowing is no longer available")]
  InvalidStep(Message),
}

impl WriteError {
  pub fn message(&self) -> Message {
    match self {
      Self::Store(error) | Self::Open(OpenError::Store(error)) => publish_error(error),
      Self::Open(OpenError::Missing) => Message::new("pipelines.problems.missing"),
      Self::InvalidStep(reason) => reason.clone(),
      Self::PersonOnly => Message::new("appMcp.errors.personOnly"),
    }
  }
}

/// Saves the draft as reviewed, only if its previous body and latest published version still match.
pub fn save_in(
  root: &Path,
  before: &OpenedPipeline,
  body: Pipeline,
  allowed: &Allowed,
) -> Result<OpenedPipeline, WriteError> {
  let name = orchester_core::name::Name::new(&before.name).expect("stored resource names are valid");
  save_named_in(root, before, body, &name, allowed)
}

/// Saves the editable display name with the draft; the stable slug and all published bodies stay intact.
pub fn save_named_in(
  root: &Path,
  before: &OpenedPipeline,
  body: Pipeline,
  name: &orchester_core::name::Name,
  allowed: &Allowed,
) -> Result<OpenedPipeline, WriteError> {
  let mut db = ProjectDb::open(root)?;
  let current = OpenedPipeline::load(&db, before.id)?;
  for step in body.steps() {
    super::narrowing::check_step(step, &current.assignables).map_err(WriteError::InvalidStep)?;
  }
  if !draft_reasons(&current, &body)
    .iter()
    .all(|reason| allowed.0.contains(reason))
  {
    return Err(WriteError::PersonOnly);
  }
  let draft = Draft::new(body, before.based_on, Timestamp::now());
  db.save_draft_and_name_matching(
    before.id,
    &before.body,
    before.latest.as_ref().map(|v| v.number),
    &draft,
    Some((&before.name, name)),
  )?;
  Ok(OpenedPipeline::load(&db, before.id)?)
}

/// Publishes the exact saved draft reviewed by the person, checking the current comparison again.
pub fn publish_in(
  root: &Path,
  id: ResourceId,
  body: &Pipeline,
  allowed: &Allowed,
) -> Result<OpenedPipeline, WriteError> {
  let mut db = ProjectDb::open(root)?;
  let current = OpenedPipeline::load(&db, id)?;
  for step in body.steps() {
    super::narrowing::check_step(step, &current.assignables).map_err(WriteError::InvalidStep)?;
  }
  let reasons = publish_reasons(current.latest.as_ref().map(|v| &v.body), body);
  if !reasons.iter().all(|reason| allowed.0.contains(reason)) {
    return Err(WriteError::PersonOnly);
  }
  db.publish_draft_matching_version(id, body, current.latest.as_ref().map(|v| v.number), Timestamp::now())?;
  Ok(OpenedPipeline::load(&db, id)?)
}

/// The next draft's number, or none at the version limit.
pub fn next_version(opened: &OpenedPipeline) -> Option<VersionNumber> {
  match opened.latest.as_ref() {
    Some(version) => version.number.next(),
    None => Some(VersionNumber::FIRST),
  }
}
/// A failed write whose saved snapshot must be read again before retrying.
pub fn needs_reload(message: &Message) -> bool {
  matches!(message.key, "errors.draftChanged" | "pipelines.problems.staleDraft")
}
