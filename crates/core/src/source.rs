//! Sources (`docs/plan.md`, M12): folders and files outside the worktree an agent may read without asking. They are
//! listed on the project, a task, a pipeline and a pipeline step (the pipeline's in its versions, like its steps); a
//! run snapshots their union when it starts ([`RunSources`]), so later edits apply to new runs, and every step of the
//! run may read all of them. Orchester's `read_external_file` and `search_external_file` then run without asking for
//! a file or folder inside one ([`ResolvedSources::covers`]); what those tools refuse anywhere (credential locations,
//! `.git`, Orchester's own state, other projects' private files) stays refused in a source too.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{permission::canonical::existing, resource::StepKey};

/// Why a path is no source.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum SourceError {
  #[error("a source must not be blank")]
  Blank,
  #[error("a source is at most {max} characters long", max = SourcePath::MAX_CHARS)]
  TooLong,
  #[error("a source must not contain control characters")]
  ControlCharacter,
  #[error("`{0}` is not an absolute path")]
  NotAbsolute(String),
}

/// The absolute path of a folder or file outside the worktree, as a person wrote it. Whether it exists is not
/// checked: it is resolved again whenever an agent starts ([`ResolvedSources::resolve`]).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SourcePath(String);

impl SourcePath {
  pub const MAX_CHARS: usize = 4096;

  pub fn new(value: &str) -> Result<Self, SourceError> {
    let value = value.trim();
    if value.is_empty() {
      return Err(SourceError::Blank);
    }
    if value.chars().count() > Self::MAX_CHARS {
      return Err(SourceError::TooLong);
    }
    if value.chars().any(char::is_control) {
      return Err(SourceError::ControlCharacter);
    }
    if !Path::new(value).is_absolute() {
      return Err(SourceError::NotAbsolute(value.to_owned()));
    }
    Ok(Self(value.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }

  pub fn path(&self) -> &Path {
    Path::new(&self.0)
  }
}

impl TryFrom<String> for SourcePath {
  type Error = SourceError;

  fn try_from(value: String) -> Result<Self, SourceError> {
    Self::new(&value)
  }
}

impl From<SourcePath> for String {
  fn from(source: SourcePath) -> Self {
    source.0
  }
}

/// Where a source of a run was listed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SourceOrigin {
  Project,
  Task,
  Pipeline,
  Step { step: StepKey },
}

/// One source of a run, with where it was listed first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunSource {
  pub path: SourcePath,
  pub origin: SourceOrigin,
}

/// The sources a run snapshotted when it started: the project's, the task's, the pinned pipeline's and its steps',
/// each path once, in that order. Every step of the run may read them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RunSources(Vec<RunSource>);

impl RunSources {
  /// The union of `listed`, in order, each path kept with the first place that lists it.
  pub fn union<'a>(listed: impl IntoIterator<Item = (&'a SourcePath, SourceOrigin)>) -> Self {
    let mut sources: Vec<RunSource> = Vec::new();
    for (path, origin) in listed {
      if !sources.iter().any(|source| &source.path == path) {
        sources.push(RunSource {
          path: path.clone(),
          origin,
        });
      }
    }
    Self(sources)
  }

  pub fn entries(&self) -> &[RunSource] {
    &self.0
  }

  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  pub fn paths(&self) -> impl Iterator<Item = &SourcePath> {
    self.0.iter().map(|source| &source.path)
  }
}

/// A run's sources resolved for one agent: links followed as far as each path exists, so a call's resolved path is
/// compared with what the source names on disk now.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResolvedSources(Vec<PathBuf>);

impl ResolvedSources {
  pub fn resolve(sources: &RunSources) -> Self {
    Self(
      sources
        .paths()
        // A source whose parents cannot be read is compared as it is written.
        .map(|source| existing(source.path()).map_or_else(|_| source.path().to_path_buf(), |found| found.path))
        .collect(),
    )
  }

  /// Whether `path` (resolved) is a source or lies in one.
  pub fn covers(&self, path: &Path) -> bool {
    self.0.iter().any(|source| path.starts_with(source))
  }
}
