//! Saving the edit the Pipelines page of a local project holds (`Pipelines · v2` P1's bar): the page keeps every change
//! to the pipeline and its steps in one [`PipelineEdit`] and stores nothing until *Save*, which stores it as the
//! pipeline's draft and publishes that draft as the next version at once; runs already started keep the version they
//! started on. The rules are the editor's: [`super::write`] saves and publishes, and [`super::widening`] decides what a
//! person has to allow.
//!
//! A save that cannot be published yet (a step without a profile or tool, a version limit) still keeps the edit as the
//! stored draft, and says why it was not published; it never says it was.

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, PipelineEdit, VersionNumber},
};

use super::{
  data::OpenedPipeline,
  hub::CYCLE_ARROW,
  problems::{problem_view, rule_message},
  widening::{draft_reasons, publish_reasons},
  write::{self, Allowed, WriteError},
};
use crate::{messages::Message, removal::discard_in};

/// What a save did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Saved {
  /// The edit is the pipeline's draft and was published as this version, or it is this latest version and the stored
  /// draft that differed from it was dropped.
  Published(VersionNumber),
  /// The edit is the pipeline's draft; it was not published, for this reason.
  Drafted(Message),
  /// The edit equals what is stored and published: nothing was written.
  Unchanged,
  /// Nothing was stored: the save widens what its agents may do, or drops an approval, and a person decides. Lists
  /// everything it needs allowed, including what the person allowed already.
  Confirm(Vec<Message>),
}

/// Saves `edit` of pipeline `id` in the project at `root`, and publishes it when it can be published. `started_from` is
/// the steps the edit was made on; the pipeline having changed since is refused (`errors.draftChanged`) and nothing is
/// stored. `allowed` is what the person already allowed to widen.
pub fn save(
  root: &Path,
  id: ResourceId,
  started_from: &Pipeline,
  edit: &PipelineEdit,
  allowed: &[Message],
) -> Result<Saved, Message> {
  let current = OpenedPipeline::load_in(root, id).map_err(|error| WriteError::from(error).message())?;
  if &current.body != started_from {
    return Err(Message::new("errors.draftChanged"));
  }
  let body = edit.validate().map_err(|error| rule_message(edit, &error))?;
  let problems = edit.problems();
  if let Some(latest) = current.latest.as_ref().filter(|latest| latest.body == body) {
    return back_to_latest(root, &current, latest.number);
  }
  let reasons = reasons(&current, &body, problems.is_empty());
  if !reasons.iter().all(|reason| allowed.contains(reason)) {
    return Ok(Saved::Confirm(reasons));
  }
  let allowed = Allowed(allowed.to_vec());
  let stored = if body == current.body && current.draft_saved_at.is_some() {
    current
  } else {
    write::save_in(root, &current, body, &allowed).map_err(|error| error.message())?
  };
  if let Some(problem) = problems.first() {
    return Ok(Saved::Drafted(problem_view(edit, problem, CYCLE_ARROW).message));
  }
  match write::publish_in(root, &stored, &allowed) {
    Ok(published) => Ok(Saved::Published(
      published
        .latest
        .expect("a pipeline that was just published has a latest version")
        .number,
    )),
    Err(error) => Ok(Saved::Drafted(error.message())),
  }
}

/// The edit is the latest version. When a different draft is stored the person chose the version over it: the draft
/// goes and the pipeline is that version, reported as saved. Otherwise the stored pipeline already is that version.
fn back_to_latest(root: &Path, current: &OpenedPipeline, latest: VersionNumber) -> Result<Saved, Message> {
  if current.draft_saved_at.is_none()
    || current
      .latest
      .as_ref()
      .is_some_and(|version| version.body == current.body)
  {
    return Ok(Saved::Unchanged);
  }
  discard_in(root, current.id, &current.body)?;
  Ok(Saved::Published(latest))
}

/// What saving `body` needs a person to allow: what saving the draft does, and what publishing it does when it can be.
fn reasons(current: &OpenedPipeline, body: &Pipeline, publishable: bool) -> Vec<Message> {
  let mut reasons = draft_reasons(current, body);
  if publishable {
    for reason in publish_reasons(current.latest.as_ref().map(|latest| &latest.body), body) {
      if !reasons.contains(&reason) {
        reasons.push(reason);
      }
    }
  }
  reasons
}

/// The edit the page holds of one pipeline: every change to the pipeline and its steps, made in memory on the steps it
/// started from, until *Save* stores it ([`save`]). *Cancel* is dropping it: nothing was stored, so what is stored is
/// what the page shows again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Held {
  base: Arc<Pipeline>,
  edit: PipelineEdit,
}

impl Held {
  /// An edit that changes nothing yet, on `base`: the steps stored now (the stored draft's, else the latest version's).
  pub fn start(base: Arc<Pipeline>) -> Self {
    Self {
      edit: PipelineEdit::new(Pipeline::clone(&base)),
      base,
    }
  }

  /// The steps the edit started from, which [`save`] compares with what is stored.
  pub fn base(&self) -> &Pipeline {
    &self.base
  }

  /// What the page shows and saves.
  pub const fn edit(&self) -> &PipelineEdit {
    &self.edit
  }

  /// Makes `change` to the edit.
  pub fn change(&mut self, change: impl FnOnce(&mut PipelineEdit)) {
    change(&mut self.edit);
  }

  /// Puts `version` into the edit, replacing every change made so far: *Save* then stores it as the next version, and
  /// the edit is no change at all when `version` is what it started from.
  pub fn restore(&mut self, version: &Pipeline) {
    self.edit = PipelineEdit::new(version.clone());
  }

  /// Whether the edit differs from what it started from: changes that cancel each other are none.
  pub fn is_changed(&self) -> bool {
    self.edit != PipelineEdit::new(Pipeline::clone(&self.base))
  }
}
