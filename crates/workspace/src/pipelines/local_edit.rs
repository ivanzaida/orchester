//! Saving the edit the Pipelines hub holds (`Pipelines · v2` P1's bar): the page keeps every change to the pipeline and
//! its steps in one [`PipelineEdit`] and stores nothing until *Save*, which saves it as the next version; runs already
//! started keep the version they started on. A local project's is stored in its database; a team project's goes to the
//! team server at once and is kept only once the server took it (D-043, [`super::write::save_named_in`]). A pipeline is
//! stored only as versions (D-032), so an edit that cannot be saved is refused and nothing is stored: it stays in the
//! page, unsaved, and the refusal names every step that keeps it. [`unpublishable`] tells the page beforehand, so
//! *Save* is not offered for it. The rules are the editor's: [`super::write`] saves, and [`super::widening`] decides
//! what a person has to allow.

use std::sync::Arc;

use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, PipelineEdit, VersionNumber},
};

use super::{
  data::Assignables,
  flow::FlowRead,
  problems::rule_message,
  ready::{Blocked, StepProblem, step_problems},
  widening::save_reasons,
  write::{self, Allowed, WriteError, save_refusals},
};
use crate::{messages::Message, sharing::Servers};

/// What a save did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Saved {
  /// The edit was published as this version.
  Published(VersionNumber),
  /// The edit equals the latest version: nothing was written.
  Unchanged,
  /// Nothing was stored: the save widens what its agents may do, or drops an approval, and a person decides. Lists
  /// everything it needs allowed, including what the person allowed already.
  Confirm(Vec<Message>),
}

/// Saves `edit` of pipeline `id` in the project at `root` as its next version; a team project's goes to its team server
/// through `servers`, and this blocks on its answer. `started_from` is the steps the edit was made on; the pipeline
/// having changed since is refused (`errors.draftChanged`), as is an edit that cannot be saved (every step that keeps
/// it named, else the first reason), and nothing is stored. `allowed` is what the person already allowed to widen.
pub fn save(
  (root, servers): (&std::path::Path, &Servers),
  id: ResourceId,
  started_from: &Pipeline,
  edit: &PipelineEdit,
  allowed: &[Message],
) -> Result<Saved, Message> {
  let opened = servers.workspace().open_pipeline(root, id);
  let current = opened.map_err(|error| WriteError::from(error).message())?;
  if &current.body != started_from {
    return Err(Message::new("errors.draftChanged"));
  }
  let body = edit.validate().map_err(|error| rule_message(edit, &error))?;
  if body == current.body {
    return Ok(Saved::Unchanged);
  }
  if let Some(first) = save_refusals(&body, &current.assignables, Some(current.version)).first() {
    let blocking = blocking_steps(&FlowRead::of_edit(edit, &current.assignables));
    return Err(every_step(&blocking).unwrap_or_else(|| first.clone()));
  }
  let reasons = save_reasons(Some(&current.body), &body, &current.assignables);
  if !reasons.iter().all(|reason| allowed.contains(reason)) {
    return Ok(Saved::Confirm(reasons));
  }
  let name = orchester_core::name::Name::new(&current.name).expect("stored resource names are valid");
  let stored = write::save_named_in(root, servers, &current, &body, &name, &Allowed(allowed.to_vec()))
    .map_err(|error| error.message())?;
  Ok(Saved::Published(stored.opened.version))
}

/// What keeps an edit from being published as it stands, so that *Save* is not offered for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unpublishable {
  /// It has no steps.
  NoSteps,
  /// Steps have no profile or tool, or name one a step cannot run.
  Steps,
  /// Its steps break a rule: they wait for each other, or for a step that is not there.
  Broken,
  /// Saving refuses a step for this, as [`super::write::save_refusals`] would (a step that writes with a profile that
  /// cannot, a narrowing its connection does not enforce).
  Refused(Message),
}

/// Why a pipeline whose latest version is `latest` takes no next version: it has the last version number.
pub fn version_limit(latest: Option<VersionNumber>) -> Option<Message> {
  latest
    .filter(|number| number.next().is_none())
    .map(|_| Message::new("pipelines.problems.versionLimit"))
}

/// What keeps `edit` from being published, its flow read as `read` with what the project's steps can run
/// (`assignables`); `None` when it can be published. The last word is the save check's own ([`save_refusals`], the
/// version limit aside), so *Save* is never offered for an edit that saving refuses.
pub fn unpublishable(edit: &PipelineEdit, read: &FlowRead, assignables: &Assignables) -> Option<Unpublishable> {
  if edit.steps().is_empty() {
    return Some(Unpublishable::NoSteps);
  }
  if !blocking_steps(read).is_empty() {
    return Some(Unpublishable::Steps);
  }
  let Ok(body) = edit.validate() else {
    return Some(Unpublishable::Broken);
  };
  save_refusals(&body, assignables, None)
    .into_iter()
    .next()
    .map(Unpublishable::Refused)
}

/// The steps of `read` saving refuses: each that cannot run, but one whose profile only lost its connection, which is
/// this computer's to fix and which saving takes.
pub fn blocking_steps(read: &FlowRead) -> Vec<StepProblem> {
  step_problems(read)
    .into_iter()
    .filter(|step| !matches!(step.blocked, Blocked::ProfileConnectionGone { .. }))
    .collect()
}

/// Why a save was refused when more than one step keeps it from being published: every one of them, by name.
fn every_step(blocking: &[StepProblem]) -> Option<Message> {
  if blocking.len() < 2 {
    return None;
  }
  let names: Vec<&str> = blocking.iter().map(|step| &*step.name).collect();
  Some(
    Message::new("pipelines.problems.stepsNotReady")
      .with("count", blocking.len().to_string())
      .with("steps", names.join(", ")),
  )
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
  /// An edit that changes nothing yet, on `base`: the steps stored now, the latest version's.
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
