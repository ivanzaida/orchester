//! Editor writes. A pipeline is stored only as a published version (D-032): saving an edit publishes it as the next
//! version, or refuses it with every reason and stores nothing. A write never publishes over a version published since
//! the edit started, nor a body other than the one reviewed.

use std::path::Path;

use orchester_core::{
  name::Name,
  resource::{FlowError, Pipeline, PublishError, ResourceBody, Step, StepKind, VersionNumber, flow_units},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::{
  data::{Assignables, OpenError, OpenedPipeline},
  local_edit::version_limit,
  problems::publish_error,
  widening::save_reasons,
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
  /// The body cannot be stored as a version ([`save_refusals`]), for every one of these reasons; nothing was stored.
  #[error("the pipeline cannot be saved as it is")]
  NotSaved(Vec<Message>),
}

impl WriteError {
  /// What a person reads: for a refused save, its first reason; [`Self::NotSaved`] holds every one.
  pub fn message(&self) -> Message {
    match self {
      Self::Store(error) | Self::Open(OpenError::Store(error)) => publish_error(error),
      Self::Open(OpenError::Missing) => Message::new("pipelines.problems.missing"),
      Self::NotSaved(reasons) => reasons
        .first()
        .cloned()
        .expect("a refused save has at least one reason"),
      Self::PersonOnly => Message::new("appMcp.errors.personOnly"),
    }
  }
}

/// Saves `body`, an edit of `before`, as the pipeline's next version under its name ([`save_named_in`]).
pub fn save_in(
  root: &Path,
  before: &OpenedPipeline,
  body: &Pipeline,
  allowed: &Allowed,
) -> Result<OpenedPipeline, WriteError> {
  let name = Name::new(&before.name).expect("stored resource names are valid");
  save_named_in(root, before, body, &name, allowed)
}

/// Saves `body`, an edit of `before`, as the pipeline's next version, renamed to `name` when that differs; the stable
/// slug and every published version stay intact. Refused, storing nothing: what [`refusals`] names, one that widens
/// beyond `allowed` ([`save_reasons`]), and one whose pipeline had a version published since `before` was read
/// (`pipelines.problems.staleDraft`). Steps equal to the latest version publish nothing ([`unchanged`]).
pub fn save_named_in(
  root: &Path,
  before: &OpenedPipeline,
  body: &Pipeline,
  name: &Name,
  allowed: &Allowed,
) -> Result<OpenedPipeline, WriteError> {
  let mut db = ProjectDb::open(root)?;
  let current = OpenedPipeline::load(&db, before.id)?;
  let refused = refusals(&current, body, name);
  if !refused.is_empty() {
    return Err(WriteError::NotSaved(refused));
  }
  let renamed = (name.as_str() != &*before.name).then_some((&*before.name, name));
  if *body == current.body {
    return unchanged(&mut db, &current, renamed);
  }
  let reasons = save_reasons(Some(&current.body), body, &current.assignables);
  if !reasons.iter().all(|reason| allowed.0.contains(reason)) {
    return Err(WriteError::PersonOnly);
  }
  db.publish_pipeline_edit(before.id, body, before.version, renamed, Timestamp::now())?;
  Ok(OpenedPipeline::load(&db, before.id)?)
}

/// Every reason a save of `body` named `name` over `opened`, its pipeline as stored, is refused, in the order a person
/// fixes them; empty when it is saved. A team pipeline's rename (its name is the team server's, and no rename is sent
/// to it), and for steps that differ from the latest version the save check ([`save_refusals`]): a rename alone
/// publishes no version, so it is not refused for the steps.
pub fn refusals(opened: &OpenedPipeline, body: &Pipeline, name: &Name) -> Vec<Message> {
  let mut refused = Vec::new();
  if opened.team && name.as_str() != &*opened.name {
    refused.push(Message::new("pipelines.problems.teamRename"));
  }
  if *body != opened.body {
    refused.extend(save_refusals(body, &opened.assignables, Some(opened.version)));
  }
  refused
}

/// A save whose steps equal the latest version, `current`: no version is published for it. One that only renames the
/// pipeline (from `renamed.0` to `renamed.1`) renames it; [`refusals`] has refused a team pipeline's rename already.
/// One that changes nothing is refused (`pipelines.problems.unchanged`).
fn unchanged(
  db: &mut ProjectDb,
  current: &OpenedPipeline,
  renamed: Option<(&str, &Name)>,
) -> Result<OpenedPipeline, WriteError> {
  let Some((_, name)) = renamed else {
    let unchanged = Message::new("pipelines.problems.unchanged").with("version", current.version.to_string());
    return Err(WriteError::NotSaved(vec![unchanged]));
  };
  db.rename_local_resource(current.id, name)?;
  Ok(OpenedPipeline::load(db, current.id)?)
}

/// The save check (D-032): every reason `body` cannot be stored as the version after `latest` (`None` for a new
/// pipeline's first), in the order a person fixes them; empty when it can. A body without steps, a step without a
/// profile or tool, a step whose executor or narrowing `assignables` cannot honour ([`refused_step`]), steps the flow
/// cannot draw as one line, and the version limit.
pub fn save_refusals(body: &Pipeline, assignables: &Assignables, latest: Option<VersionNumber>) -> Vec<Message> {
  let mut refusals = Vec::new();
  let mut add = |reason: Message| {
    if !refusals.contains(&reason) {
      refusals.push(reason);
    }
  };
  if let Err(PublishError::NoSteps) = body.check_publishable() {
    add(Message::new("pipelines.problems.noSteps"));
  }
  for step in body.steps() {
    if let Some(unassigned) = unassigned(step) {
      add(unassigned);
    }
    if let Err(refused) = super::narrowing::check_step(step, assignables) {
      add(refused);
    }
  }
  if let Err(FlowError::Branches(key)) = flow_units(body.steps()) {
    let name = body
      .step(&key)
      .map_or_else(|| key.as_str().to_owned(), |step| step.name.as_str().to_owned());
    add(Message::new("pipelines.problems.branches").with("step", name));
  }
  if let Some(limit) = version_limit(latest) {
    add(limit);
  }
  refusals
}

/// Why `step` has nothing to run: no profile, or no tool; `None` when it names one.
fn unassigned(step: &Step) -> Option<Message> {
  let key = match step.kind {
    StepKind::Agent { profile: None, .. } => "pipelines.problems.noProfile",
    StepKind::Tool { tool: None } => "pipelines.problems.noTool",
    StepKind::Agent { .. } | StepKind::Tool { .. } => return None,
  };
  Some(Message::new(key).with("step", step.name.as_str().to_owned()))
}

/// What saving `steps` refuses in one of them now, the first: an executor or a narrowing `assignables` cannot honour
/// ([`super::narrowing::check_step`]). The page says it before *Save* ([`super::local_edit::unpublishable`]); the save
/// check names every one ([`save_refusals`]).
pub fn refused_step(steps: &[Step], assignables: &Assignables) -> Option<Message> {
  steps
    .iter()
    .find_map(|step| super::narrowing::check_step(step, assignables).err())
}

/// The number a save of `opened` would publish, or none at the version limit.
pub fn next_version(opened: &OpenedPipeline) -> Option<VersionNumber> {
  opened.version.next()
}

/// A failed write whose stored pipeline must be read again before retrying.
pub fn needs_reload(message: &Message) -> bool {
  matches!(message.key, "errors.draftChanged" | "pipelines.problems.staleDraft")
}
