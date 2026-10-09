//! The project's pipelines as the library lists them (03.04 `LRMQW`), and creating a new one
//! (03.05 `c3jD7`, 03.A `MqRT7`). Every function blocks on SQLite; the UI calls them off the UI
//! thread.

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{
    Pipeline, ResourceKind, Slug, StarterAssignments, StarterStep, StarterStepText, VersionNumber, feature_delivery,
  },
  store::{ResourceStore, TaskStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::{data::Assignables, layout::levels};
use crate::messages::{self, Message};

/// Where a pipeline is in its lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VersionState {
  /// Never published: tasks cannot use it yet.
  DraftOnly,
  Published(VersionNumber),
  /// Published, with unpublished changes saved as the next draft.
  PublishedWithDraft(VersionNumber),
}

/// One row of the library.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryEntry {
  pub id: ResourceId,
  pub name: Arc<str>,
  pub slug: Arc<str>,
  /// Step names by dependency level: steps of one level run in parallel.
  pub stages: Vec<Vec<Arc<str>>>,
  pub step_count: usize,
  pub state: VersionState,
  pub used_by: usize,
  pub last_published: Option<Timestamp>,
}

impl lurq::app::component::DevtoolsInspectable for LibraryEntry {}

/// The project's pipelines, ordered by slug.
pub fn library(db: &ProjectDb) -> Result<Vec<LibraryEntry>, StoreError> {
  let tasks = db.tasks()?;
  let mut entries = Vec::new();
  for resource in db.resources(ResourceKind::Pipeline)? {
    let id = resource.id();
    let latest = db.latest_version::<Pipeline>(id)?;
    let draft = db.draft::<Pipeline>(id)?;
    let (body, state) = match (draft, &latest) {
      (Some(draft), None) => (draft.body().clone(), VersionState::DraftOnly),
      (Some(draft), Some(version)) => (draft.body().clone(), VersionState::PublishedWithDraft(version.number())),
      (None, Some(version)) => (version.body().clone(), VersionState::Published(version.number())),
      (None, None) => continue,
    };
    let names = |keys: Vec<usize>| {
      keys
        .into_iter()
        .map(|index| body.steps()[index].name.as_str().into())
        .collect()
    };
    entries.push(LibraryEntry {
      id,
      name: resource.name().as_str().into(),
      slug: resource.slug().as_str().into(),
      stages: levels(body.steps()).into_iter().map(names).collect(),
      step_count: body.steps().len(),
      state,
      used_by: tasks.iter().filter(|task| task.details().pipeline == Some(id)).count(),
      last_published: latest.map(|version| version.published_at()),
    });
  }
  Ok(entries)
}

pub fn library_in(repo_root: &Path) -> Result<Vec<LibraryEntry>, StoreError> {
  library(&ProjectDb::open(repo_root)?)
}

/// What a new pipeline starts with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartFrom {
  Starter,
  Empty,
}

impl lurq::app::component::DevtoolsInspectable for StartFrom {}

/// Translated text the starter needs: each step's name and instructions, and the name of the
/// profile or tool it looks for in the project, e.g. "Explorer" for Research.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StarterText {
  steps: Vec<(StarterStepText, String)>,
}

impl StarterText {
  pub fn new(text: impl Fn(StarterStep) -> (StarterStepText, String)) -> Self {
    Self {
      steps: StarterStep::ALL.into_iter().map(text).collect(),
    }
  }

  fn entry(&self, step: StarterStep) -> &(StarterStepText, String) {
    let index = StarterStep::ALL
      .iter()
      .position(|candidate| *candidate == step)
      .expect("every starter step is in ALL");
    &self.steps[index]
  }

  /// The starter pipeline with the project's profiles and tools of the looked-for names.
  pub fn pipeline(&self, assignables: &Assignables) -> Pipeline {
    let executor = |step| self.entry(step).1.as_str();
    feature_delivery(
      |step| self.entry(step).0.clone(),
      starter_assignments(assignables, &executor),
    )
  }
}

/// A new pipeline to create.
pub struct NewPipeline {
  pub name: Name,
  pub start: StartFrom,
  pub starter: StarterText,
}

#[derive(Debug, thiserror::Error)]
pub enum CreateError {
  #[error(transparent)]
  Store(#[from] StoreError),
  /// The name has no letter or digit a slug could be made of.
  #[error("the name gives no slug")]
  NoSlug,
  /// Another pipeline already has the slug the name gives.
  #[error("the slug `{slug}` is used by `{existing}`")]
  SlugTaken { slug: String, existing: String },
}

impl CreateError {
  /// What a person reads when the pipeline is not created.
  pub fn message(&self) -> Message {
    match self {
      Self::Store(error) => messages::store_error(error),
      Self::NoSlug => Message::new("pipelines.create.noSlug"),
      Self::SlugTaken { slug, existing } => slug_taken(slug, existing),
    }
  }
}

/// The name gives slug `slug`, which pipeline `existing` has.
pub fn slug_taken(slug: &str, existing: &str) -> Message {
  Message::new("pipelines.create.slugTaken")
    .with("slug", slug.to_owned())
    .with("pipeline", existing.to_owned())
}

/// The slug a new pipeline named `name` would get, or why it cannot be created with that name.
pub fn slug_for<S: ResourceStore>(store: &S, name: &str) -> Result<Slug, CreateError>
where
  CreateError: From<S::Error>,
{
  let slug = Slug::from_name(name).ok_or(CreateError::NoSlug)?;
  let existing = store
    .resources(ResourceKind::Pipeline)?
    .into_iter()
    .find(|resource| resource.slug() == &slug);
  match existing {
    Some(resource) => Err(CreateError::SlugTaken {
      slug: slug.as_str().to_owned(),
      existing: resource.name().as_str().to_owned(),
    }),
    None => Ok(slug),
  }
}

/// Creates the pipeline as draft v1 and returns its id. The starter's steps use the project's
/// published profiles and tools with the names the starter looks for; the others stay unset,
/// and the editor says what is missing before it can be published. The slug is unique, so
/// repeating a create that did happen fails with [`CreateError::SlugTaken`] instead of adding
/// a second pipeline.
pub fn create(db: &mut ProjectDb, new: NewPipeline, now: Timestamp) -> Result<ResourceId, CreateError> {
  let body = match new.start {
    StartFrom::Empty => Pipeline::new(Vec::new()).expect("a pipeline without steps is a valid draft"),
    StartFrom::Starter => new.starter.pipeline(&Assignables::load(db)?),
  };
  create_with(db, new.name, &body, now)
}

/// Creates pipeline `name` with `body` as its draft v1, as [`create`] does with a start.
pub fn create_with(db: &mut ProjectDb, name: Name, body: &Pipeline, now: Timestamp) -> Result<ResourceId, CreateError> {
  let slug = slug_for(db, name.as_str())?;
  Ok(db.create_resource(slug, name, body, now)?.id())
}

pub fn create_in(repo_root: &Path, new: NewPipeline) -> Result<ResourceId, CreateError> {
  create(&mut ProjectDb::open(repo_root)?, new, Timestamp::now())
}

/// The published profiles and tools whose names the starter looks for.
pub fn starter_assignments<'a>(
  assignables: &Assignables,
  executor: &dyn Fn(StarterStep) -> &'a str,
) -> StarterAssignments {
  let profile = |step| assignables.profile_named(executor(step));
  let tool = |step| assignables.tool_named(executor(step));
  StarterAssignments {
    explorer: profile(StarterStep::Research),
    planner: profile(StarterStep::Plan),
    builder: profile(StarterStep::Implement),
    reviewer: profile(StarterStep::Review),
    checks: tool(StarterStep::RunChecks),
    pull_request: tool(StarterStep::CreatePullRequest),
  }
}
