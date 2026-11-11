//! Editor writes. A pipeline is stored only as a published version (D-032): saving an edit publishes it as the next
//! version, or refuses it with every reason and stores nothing. A write never publishes over a version published since
//! the edit started, nor a body other than the one reviewed. A team pipeline's save goes to the team server at once and
//! is kept here only once the server took it (D-043, [`super::team_write`]); a local one is stored here.

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
  team_write,
  widening::save_reasons,
};
use crate::{
  messages::Message,
  sharing::{NotTaken, Servers},
};

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
  /// The team server did not take the team pipeline's change; nothing of it was stored.
  #[error("the team server did not take the pipeline's change")]
  Team(NotTaken),
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
      Self::Team(refused) => refused.message(),
    }
  }
}

/// What a save stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stored {
  /// The pipeline as it is stored now.
  pub opened: OpenedPipeline,
  /// Why a team pipeline's steps were not saved when its new name was: the rename is sent first, so it is kept, and
  /// only the steps are left to save again.
  pub steps_not_saved: Option<Message>,
}

/// A save that passed its checks: what it changes of `current`, the pipeline as stored now.
pub(super) struct Checked {
  pub current: OpenedPipeline,
  /// The new name, when it differs.
  pub renamed: Option<Name>,
  /// The steps to publish as the next version, when they differ from the latest version.
  pub steps: Option<Pipeline>,
}

/// Saves `body`, an edit of `before`, as the pipeline's next version, renamed to `name` when that differs; the stable
/// slug and every published version stay intact. Refused, storing nothing: what [`refusals`] names, one that widens
/// beyond `allowed` ([`save_reasons`]), one whose pipeline had a version published since `before` was read
/// (`pipelines.problems.staleDraft`), and one that changes nothing (`pipelines.problems.unchanged`). Steps equal to the
/// latest version publish nothing: a rename alone renames. A team pipeline's goes to the team server through `servers`
/// ([`team_write::save`]); this blocks on its answer, so it runs on a worker of the application's runtime.
pub fn save_named_in(
  root: &Path,
  servers: &Servers,
  before: &OpenedPipeline,
  body: &Pipeline,
  name: &Name,
  allowed: &Allowed,
) -> Result<Stored, WriteError> {
  let mut db = ProjectDb::open(root)?;
  let checked = check(&db, before, body, name, allowed)?;
  if checked.current.team {
    drop(db);
    return team_write::save(servers, root, before, checked);
  }
  Ok(Stored {
    opened: store_here(&mut db, before, &checked)?,
    steps_not_saved: None,
  })
}

/// Saves `body`, an edit of `before`, named `name`, as a pipeline of a local project, as [`save_named_in`] does; a team
/// pipeline, which only the team server takes, is refused ([`StoreError::SharedProject`]) and nothing is stored. For
/// the Pipelines hub, which shows a local project only.
pub fn save_here_in(
  root: &Path,
  before: &OpenedPipeline,
  body: &Pipeline,
  name: &Name,
  allowed: &Allowed,
) -> Result<OpenedPipeline, WriteError> {
  let mut db = ProjectDb::open(root)?;
  let checked = check(&db, before, body, name, allowed)?;
  if checked.current.team {
    return Err(StoreError::SharedProject.into());
  }
  store_here(&mut db, before, &checked)
}

/// Stores `checked`, a save of `before`, in `db`: its steps as the next version, renamed with them, or its name alone.
fn store_here(db: &mut ProjectDb, before: &OpenedPipeline, checked: &Checked) -> Result<OpenedPipeline, WriteError> {
  if let Some(steps) = &checked.steps {
    let names = checked.renamed.as_ref().map(|name| (&*before.name, name));
    db.publish_pipeline_edit(before.id, steps, before.version, names, Timestamp::now())?;
  } else if let Some(name) = &checked.renamed {
    db.rename_local_resource(before.id, name)?;
  }
  Ok(OpenedPipeline::load(db, before.id)?)
}

/// The checks of a save of `body` named `name` over `before`, against the pipeline as stored in `db`: what it changes,
/// or why it is refused.
fn check(
  db: &ProjectDb,
  before: &OpenedPipeline,
  body: &Pipeline,
  name: &Name,
  allowed: &Allowed,
) -> Result<Checked, WriteError> {
  let current = OpenedPipeline::load(db, before.id)?;
  let refused = refusals(&current, body);
  if !refused.is_empty() {
    return Err(WriteError::NotSaved(refused));
  }
  let renamed = (name.as_str() != &*before.name).then(|| name.clone());
  let steps = (*body != current.body).then(|| body.clone());
  if renamed.is_none() && steps.is_none() {
    let unchanged = Message::new("pipelines.problems.unchanged").with("version", current.version.to_string());
    return Err(WriteError::NotSaved(vec![unchanged]));
  }
  if steps.is_some() {
    let reasons = save_reasons(Some(&current.body), body, &current.assignables);
    if !reasons.iter().all(|reason| allowed.0.contains(reason)) {
      return Err(WriteError::PersonOnly);
    }
  }
  Ok(Checked {
    current,
    renamed,
    steps,
  })
}

/// Every reason a save of `body` over `opened`, its pipeline as stored, is refused, in the order a person fixes them;
/// empty when it is saved. Steps that differ from the latest version take the save check ([`save_refusals`]): a rename
/// alone publishes no version, so it is not refused for the steps.
pub fn refusals(opened: &OpenedPipeline, body: &Pipeline) -> Vec<Message> {
  if *body == opened.body {
    return Vec::new();
  }
  save_refusals(body, &opened.assignables, Some(opened.version))
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
