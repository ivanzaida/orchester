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

use super::{data::Assignables, levels::levels, write::save_refusals};
use crate::{
  messages::{self, Message},
  sharing::{NotTaken, Servers},
};

/// One row of the library: a pipeline and its latest version. A pipeline is stored only as published versions (D-032).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryEntry {
  pub id: ResourceId,
  pub name: Arc<str>,
  pub slug: Arc<str>,
  /// Step names by dependency level: steps of one level run in parallel.
  pub stages: Vec<Vec<Arc<str>>>,
  pub step_count: usize,
  /// Its latest version.
  pub version: VersionNumber,
  pub used_by: usize,
  /// When its latest version was published.
  pub last_published: Timestamp,
  /// The project's default pipeline, which New task starts with.
  pub default: bool,
}

/// The project's pipelines it offers (in a team project only the team's, `ProjectDb::resources`), ordered by slug. One
/// without a version used here is left out.
pub fn library(db: &ProjectDb) -> Result<Vec<LibraryEntry>, StoreError> {
  let tasks = db.tasks()?;
  let default = db.default_pipeline()?;
  let mut entries = Vec::new();
  for resource in db.resources(ResourceKind::Pipeline)? {
    let id = resource.id();
    let Some(latest) = db.latest_version::<Pipeline>(id)? else {
      continue;
    };
    let body = latest.body();
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
      version: latest.number(),
      used_by: tasks.iter().filter(|task| task.details().pipeline == Some(id)).count(),
      last_published: latest.published_at(),
      default: default == Some(id),
    });
  }
  Ok(entries)
}

pub fn library_in(repo_root: &Path) -> Result<Vec<LibraryEntry>, StoreError> {
  library(&ProjectDb::open(repo_root)?)
}

/// Makes `pipeline` the project's default, the one New task starts with. Only a published pipeline can be: New task
/// lists no other.
pub fn make_default_in(repo_root: &Path, pipeline: ResourceId) -> Result<(), MakeDefaultError> {
  let mut db = ProjectDb::open(repo_root)?;
  if db.latest_version::<Pipeline>(pipeline)?.is_none() {
    return Err(MakeDefaultError::NotPublished);
  }
  Ok(db.save_default_pipeline(Some(pipeline))?)
}

#[derive(Debug, thiserror::Error)]
pub enum MakeDefaultError {
  #[error(transparent)]
  Store(#[from] StoreError),
  /// The pipeline has no published version: New task could not choose it.
  #[error("the pipeline is not published")]
  NotPublished,
}

impl MakeDefaultError {
  /// What a person reads when the pipeline is not made the default.
  pub fn message(&self) -> Message {
    match self {
      Self::Store(error) => messages::store_error(error),
      Self::NotPublished => Message::new("pipelines.default.notPublished"),
    }
  }
}

/// What a new pipeline starts with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartFrom {
  Starter,
  Empty,
}

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
  /// The body cannot be stored as a first version ([`save_refusals`]), for every one of these reasons; nothing was
  /// created.
  #[error("the pipeline cannot be saved as it is")]
  NotSaved(Vec<Message>),
  /// The team server did not take a team pipeline; nothing of it was stored.
  #[error("the team server did not take the pipeline")]
  Team(NotTaken),
}

impl CreateError {
  /// What a person reads when the pipeline is not created: for a refused body, its first reason.
  pub fn message(&self) -> Message {
    match self {
      Self::Store(error) => messages::store_error(error),
      Self::NoSlug => Message::new("pipelines.create.noSlug"),
      Self::SlugTaken { slug, existing } => slug_taken(slug, existing),
      Self::NotSaved(reasons) => reasons
        .first()
        .cloned()
        .expect("a refused create has at least one reason"),
      Self::Team(refused) => refused.message(),
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

/// Creates the pipeline as its first version and returns its id, or refuses a body the save check refuses
/// ([`CreateError::NotSaved`]): one without steps always is. The starter's steps use the project's published profiles
/// and tools with the names the starter looks for; while one is not found its step has none, and the starter is
/// refused. The slug is unique, so repeating a create that did happen fails with [`CreateError::SlugTaken`] instead of
/// adding a second pipeline. The project at `at`'s path, with its servers, is the one `db` holds ([`create_with`]).
pub fn create(
  db: &mut ProjectDb,
  at: (&Path, &Servers),
  new: NewPipeline,
  now: Timestamp,
) -> Result<ResourceId, CreateError> {
  let body = match new.start {
    StartFrom::Empty => Pipeline::new(Vec::new()).expect("a pipeline without steps is a valid body"),
    StartFrom::Starter => new.starter.pipeline(&project_assignables(db, at)?),
  };
  create_with(db, at, new.name, &body, now)
}

/// Creates pipeline `name` with `body` as its first version, as [`create`] does with a start. In a team project, the
/// one at `at`'s path that `db` holds, it is sent to the team server through `at`'s servers at once and kept only once
/// the server made it (D-043): this blocks on the answer, so it runs on a worker of the application's runtime.
pub fn create_with(
  db: &mut ProjectDb,
  (root, servers): (&Path, &Servers),
  name: Name,
  body: &Pipeline,
  now: Timestamp,
) -> Result<ResourceId, CreateError> {
  if db.sharing()?.is_none() {
    return create_here(db, name, body, now);
  }
  let slug = team_checked(db, (root, servers), &name, body)?;
  servers
    .create_team_pipeline(root, (slug, name), body)
    .map_err(CreateError::Team)
}

/// What the steps of a pipeline of the project `db` holds, at `at`'s path, can run: a local project's published
/// profiles and tools, a team project's from the team's configuration as this session read it. While that is being
/// read, or could not be read, nothing is created, with why.
pub(super) fn project_assignables(
  db: &ProjectDb,
  (root, servers): (&Path, &Servers),
) -> Result<Assignables, CreateError> {
  if db.sharing()?.is_none() {
    return Ok(Assignables::load(db)?);
  }
  let config = servers
    .team_read(root)
    .loaded()
    .map_err(|unread| CreateError::NotSaved(vec![unread.message()]))?;
  Ok(Assignables::of_team(db, &config)?)
}

/// The slug of a new team pipeline `name` with `body` as its first version, or why it is not sent: a name another of
/// the team's pipelines has the slug of, and a body the save check refuses for what the team's profiles and tools are
/// on this computer. The team server has the last word on the slug.
fn team_checked(
  db: &ProjectDb,
  (root, servers): (&Path, &Servers),
  name: &Name,
  body: &Pipeline,
) -> Result<Slug, CreateError> {
  let config = servers
    .team_read(root)
    .loaded()
    .map_err(|unread| CreateError::NotSaved(vec![unread.message()]))?;
  let slug = Slug::from_name(name.as_str()).ok_or(CreateError::NoSlug)?;
  let taken = config
    .resources(ResourceKind::Pipeline)
    .into_iter()
    .find(|resource| resource.slug == slug.as_str());
  if let Some(existing) = taken {
    return Err(CreateError::SlugTaken {
      slug: slug.as_str().to_owned(),
      existing: existing.name.as_str().to_owned(),
    });
  }
  let refusals = save_refusals(body, &Assignables::of_team(db, &config)?, None);
  if !refusals.is_empty() {
    return Err(CreateError::NotSaved(refusals));
  }
  Ok(slug)
}

/// Creates pipeline `name` with `body` as its first version in a local project, as [`create_with`] does there. A team
/// project's new pipeline is the team server's to make, so it is refused here ([`StoreError::SharedProject`]) and
/// nothing is stored.
pub fn create_here(db: &mut ProjectDb, name: Name, body: &Pipeline, now: Timestamp) -> Result<ResourceId, CreateError> {
  let slug = checked(db, &name, body)?;
  if db.sharing()?.is_some() {
    return Err(StoreError::SharedProject.into());
  }
  Ok(db.create_published(slug, name, body, now)?.0.id())
}

/// The slug of a new pipeline `name` with `body` as its first version, or why it cannot be created.
fn checked(db: &ProjectDb, name: &Name, body: &Pipeline) -> Result<Slug, CreateError> {
  let slug = slug_for(db, name.as_str())?;
  let refusals = save_refusals(body, &Assignables::load(db)?, None);
  if !refusals.is_empty() {
    return Err(CreateError::NotSaved(refusals));
  }
  Ok(slug)
}

pub fn create_in(repo_root: &Path, servers: &Servers, new: NewPipeline) -> Result<ResourceId, CreateError> {
  create(
    &mut ProjectDb::open(repo_root)?,
    (repo_root, servers),
    new,
    Timestamp::now(),
  )
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
/// The library pipeline whose slug the entered name would use, for creation feedback.
pub fn matching_name<'a>(entries: &'a [LibraryEntry], name: &str) -> Option<&'a LibraryEntry> {
  let slug = Slug::from_name(name)?;
  entries.iter().find(|entry| entry.slug.as_ref() == slug.as_str())
}
