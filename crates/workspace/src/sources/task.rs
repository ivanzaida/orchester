//! A task's sources (New task's and Edit task's *Sources* under Context, `zcFQJ`): its own, on top of the project's
//! and those of the pipeline version it runs (the pipeline's and its steps'), which the form shows read-only. A run
//! of the task starts with their union ([`run_count`]). Adding a source of the task's own is a person's to confirm,
//! as for the project's ([`add_in`], [`save_new_in`]). Blocks on SQLite and reads the file system; the UI calls it
//! off the UI thread.

use std::path::Path;

use orchester_core::{
  id::{ResourceId, TaskId},
  resource::{Pipeline, VersionNumber},
  source::{RunSources, SourceOrigin, SourcePath},
  store::ResourceStore,
};
use orchester_store::{ProjectDb, StoreError};

use super::{
  check::Place,
  listed::{Listed, inspect_all},
  project::{Removed, reading},
};
use crate::messages::{self, Message};

/// A source the task gets from the pipeline version it runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FromPipeline {
  pub listed: Listed,
  pub pipeline: String,
  pub version: VersionNumber,
  /// The step that lists it; `None` for the pipeline's own.
  pub step: Option<String>,
}

impl FromPipeline {
  /// Where it is listed (`05 Review in Feature delivery v3`).
  pub fn origin(&self) -> Message {
    let version = self.version.get().to_string();
    match &self.step {
      Some(step) => Message::new("sources.origin.step")
        .with("step", step.clone())
        .with("pipeline", self.pipeline.clone())
        .with("version", version),
      None => Message::new("sources.origin.pipeline")
        .with("pipeline", self.pipeline.clone())
        .with("version", version),
    }
  }
}

/// What a task gets besides its own sources.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Inherited {
  pub project: Vec<Listed>,
  pub pipeline: Vec<FromPipeline>,
}

impl Inherited {
  fn paths(&self) -> impl Iterator<Item = &SourcePath> {
    self
      .project
      .iter()
      .map(|listed| &listed.path)
      .chain(self.pipeline.iter().map(|from| &from.listed.path))
  }
}

/// The project's sources and those of `pipeline`'s published version, as they are on disk now.
pub fn inherited_in(repo_root: &Path, pipeline: Option<(ResourceId, VersionNumber)>) -> Result<Inherited, StoreError> {
  let db = ProjectDb::open(repo_root)?;
  let mut inherited = Inherited {
    project: inspect_all(&db.project_sources()?),
    pipeline: Vec::new(),
  };
  let Some((id, number)) = pipeline else {
    return Ok(inherited);
  };
  let (Some(resource), Some(version)) = (db.resource(id)?, db.version::<Pipeline>(id, number)?) else {
    return Ok(inherited);
  };
  let body = version.body();
  let from = |listed: Listed, step: Option<String>| FromPipeline {
    listed,
    pipeline: resource.name().as_str().to_owned(),
    version: number,
    step,
  };
  for path in body.sources().pipeline() {
    inherited.pipeline.push(from(Listed::inspect(path), None));
  }
  for step in body.steps() {
    for path in body.sources().step(&step.key) {
      inherited
        .pipeline
        .push(from(Listed::inspect(path), Some(step.name.as_str().to_owned())));
    }
  }
  Ok(inherited)
}

/// The sources `task` lists itself.
pub fn own_in(repo_root: &Path, task: TaskId) -> Result<Vec<SourcePath>, StoreError> {
  ProjectDb::open(repo_root)?.task_sources(task)
}

/// Where a path for the task's own sources is checked: outside the project and every source a run of it reads.
pub fn place(repo_root: &Path, project: &str, inherited: &Inherited, own: &[SourcePath]) -> Place {
  Place::new(repo_root, project, inherited.paths().chain(own).cloned().collect())
}

/// How many sources a run of the task starts with: each path once.
pub fn run_count(inherited: &Inherited, own: &[SourcePath]) -> usize {
  RunSources::union(
    inherited
      .project
      .iter()
      .map(|listed| (&listed.path, SourceOrigin::Project))
      .chain(own.iter().map(|path| (path, SourceOrigin::Task)))
      .chain(
        inherited
          .pipeline
          .iter()
          .map(|from| (&from.listed.path, SourceOrigin::Pipeline)),
      ),
  )
  .entries()
  .len()
}

/// What adding `path` to a task lets its agents do, for the person to confirm.
pub fn widenings(path: &SourcePath) -> Vec<Message> {
  vec![Message::new("sources.widens.task").with("path", path.as_str().to_owned())]
}

/// Adds `path` to the sources of `task` once a person `confirmed` it, checked again as it is stored.
pub fn add_in(
  repo_root: &Path,
  task: TaskId,
  pipeline: Option<(ResourceId, VersionNumber)>,
  path: &SourcePath,
  confirmed: bool,
) -> Result<(), Message> {
  if !confirmed {
    return Err(Message::new("sources.personOnly"));
  }
  let store = |error: StoreError| messages::store_error(&error);
  let inherited = inherited_in(repo_root, pipeline).map_err(store)?;
  let mut db = ProjectDb::open(repo_root).map_err(store)?;
  let mut own = db.task_sources(task).map_err(store)?;
  place(repo_root, db.project().name().as_str(), &inherited, &own)
    .check(path.as_str())
    .map_err(|refusal| refusal.message())?;
  own.push(path.clone());
  db.set_task_sources(task, &own).map_err(store)
}

/// Takes `path` off the sources of `task`; narrowing, so no one is asked.
pub fn remove_in(repo_root: &Path, task: TaskId, path: &SourcePath) -> Result<Removed, Message> {
  let store = |error: StoreError| messages::store_error(&error);
  let mut db = ProjectDb::open(repo_root).map_err(store)?;
  let mut own = db.task_sources(task).map_err(store)?;
  own.retain(|source| source != path);
  db.set_task_sources(task, &own).map_err(store)?;
  Ok(Removed {
    path: path.clone(),
    runs: reading(&db, path).map_err(store)?,
  })
}

/// Stores `sources` as the sources of the new `task`, each added once a person `confirmed` it on the form.
pub fn save_new_in(repo_root: &Path, task: TaskId, sources: &[SourcePath], confirmed: bool) -> Result<(), Message> {
  if sources.is_empty() {
    return Ok(());
  }
  if !confirmed {
    return Err(Message::new("sources.personOnly"));
  }
  ProjectDb::open(repo_root)
    .and_then(|mut db| db.set_task_sources(task, sources))
    .map_err(|error| messages::store_error(&error))
}
