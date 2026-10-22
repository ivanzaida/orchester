//! The project's sources (Project settings › Sources `kmNiF`): every run of the project reads them. Adding one lets
//! agents read more without asking, so it is a person's to confirm ([`add_in`] refuses it unconfirmed); removing one
//! only narrows, and the runs that started with it keep it until they end ([`Removed`]). Blocks on SQLite and reads
//! the file system; the UI calls it off the UI thread.

use std::path::Path;

use orchester_core::{source::SourcePath, store::TaskStore};
use orchester_store::{ProjectDb, StoreError};

use super::{
  check::Place,
  listed::{Listed, inspect_all},
};
use crate::{
  messages::{self, Message},
  tasks::detail::run_store_error,
};

/// The project's sources as they are on disk now.
pub fn load_in(repo_root: &Path) -> Result<Vec<Listed>, StoreError> {
  let db = ProjectDb::open(repo_root)?;
  Ok(inspect_all(&db.project_sources()?))
}

/// Where a path for the project's sources is checked: outside the project and its sources.
pub fn place_in(repo_root: &Path) -> Result<Place, StoreError> {
  let db = ProjectDb::open(repo_root)?;
  Ok(Place::new(
    repo_root,
    db.project().name().as_str(),
    db.project_sources()?,
  ))
}

/// What adding `path` lets agents do, for the person to confirm. Shared by the window and automation.
pub fn widenings(path: &SourcePath) -> Vec<Message> {
  vec![Message::new("sources.widens.project").with("path", path.as_str().to_owned())]
}

/// Adds `path` to the project's sources once a person `confirmed` it, checked again as it is stored.
pub fn add_in(repo_root: &Path, path: &SourcePath, confirmed: bool) -> Result<(), Message> {
  if !confirmed {
    return Err(Message::new("sources.personOnly"));
  }
  let mut db = ProjectDb::open(repo_root).map_err(|error| messages::store_error(&error))?;
  let mut listed = db.project_sources().map_err(|error| messages::store_error(&error))?;
  Place::new(repo_root, db.project().name().as_str(), listed.clone())
    .check(path.as_str())
    .map_err(|refusal| refusal.message())?;
  listed.push(path.clone());
  db.set_project_sources(&listed)
    .map_err(|error| messages::store_error(&error))
}

/// A source taken off a list, and the runs that started with it, which keep reading it until they end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Removed {
  pub path: SourcePath,
  /// Each run as its task's key and its number (`KON-12`, `#128`).
  pub runs: Vec<(String, String)>,
}

impl Removed {
  /// What happens now, under *Removed …* (`Notice · Removed` `UWffE`).
  pub fn message(&self) -> Message {
    match self.runs.as_slice() {
      [] => Message::new("sources.removed.noRun"),
      [(task, run)] => Message::new("sources.removed.oneRun")
        .with("task", task.clone())
        .with("run", run.clone()),
      runs => Message::new("sources.removed.runs").with("count", runs.len().to_string()),
    }
  }
}

/// Takes `path` off the project's sources; narrowing, so no one is asked.
pub fn remove_in(repo_root: &Path, path: &SourcePath) -> Result<Removed, Message> {
  let mut db = ProjectDb::open(repo_root).map_err(|error| messages::store_error(&error))?;
  let mut listed = db.project_sources().map_err(|error| messages::store_error(&error))?;
  listed.retain(|source| source != path);
  db.set_project_sources(&listed)
    .map_err(|error| messages::store_error(&error))?;
  let runs = reading(&db, path).map_err(|error| messages::store_error(&error))?;
  Ok(Removed {
    path: path.clone(),
    runs,
  })
}

/// The runs that have not ended and started with `path` among their sources.
pub(super) fn reading(db: &ProjectDb, path: &SourcePath) -> Result<Vec<(String, String)>, StoreError> {
  let mut runs = Vec::new();
  for run in db.unfinished_runs().map_err(run_store_error)? {
    // A run that cannot be read is not named: what it reads is not known.
    let Ok(run) = run else {
      continue;
    };
    if run.snapshot().sources().paths().any(|source| source == path) {
      let key = db
        .task(run.task())?
        .map(|task| task.key().to_string())
        .unwrap_or_default();
      runs.push((key, run.number().to_string()));
    }
  }
  Ok(runs)
}
