//! What the code viewer reads off the UI thread: the version's files, the file, the versions it has, and
//! the file's highlighted lines. A read's result names what it was read for, since a future keeps its last
//! result while it reads again; a result for other arguments is not shown. The results' DevTools entries
//! are empty, so a file's text never reaches the DevTools tree.

use std::{path::Path, sync::Arc};

use lurq::app::{component::DevtoolsInspectable, ctx::Ctx};
use orchester_core::id::RunId;
use orchester_workspace::{
  background,
  files::{FileList, FilePath, FileText, FileView, FilesError, Version, Versions, list_files, read_file, versions},
  highlight::{LineTokens, highlight_lines},
  messages::Message,
};

use crate::services::Services;

/// Why a read failed: the files' own reason, or the background work that did not finish.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
  Files(FilesError),
  Interrupted(Message),
}

impl DevtoolsInspectable for Failure {}

impl Failure {
  pub fn message(&self) -> Message {
    match self {
      Self::Files(error) => error.message(),
      Self::Interrupted(message) => message.clone(),
    }
  }

  /// The version read no longer exists.
  pub const fn gone(&self) -> bool {
    matches!(self, Self::Files(FilesError::VersionGone(_)))
  }
}

/// A read's value with what it was read for.
#[derive(Clone, PartialEq)]
pub struct Found<K, T> {
  pub key: K,
  pub value: T,
}

impl<K, T> DevtoolsInspectable for Found<K, T> {}

/// What a read shows now: its value for the current arguments, or its failure, or that it is under way.
#[derive(Clone, PartialEq)]
pub enum Shown<T> {
  Reading,
  Read(T),
  Failed(Failure),
}

impl<T> Shown<T> {
  pub const fn value(&self) -> Option<&T> {
    match self {
      Self::Read(value) => Some(value),
      Self::Reading | Self::Failed(_) => None,
    }
  }
}

/// The result of a future for `key`: its value when it was read for `key`, else its failure, else that it
/// is being read.
fn shown<K: PartialEq, T: Clone>(key: &K, state: &lurq::app::ctx::FutureState<Found<K, T>, Failure>) -> Shown<T> {
  if let Some(found) = state.data.as_ref()
    && found.key == *key
    && !state.is_rejected()
  {
    return Shown::Read(found.value.clone());
  }
  match (&state.error, state.is_pending()) {
    (Some(error), false) => Shown::Failed(error.clone()),
    _ => Shown::Reading,
  }
}

async fn run<T: Send + 'static>(work: impl FnOnce() -> Result<T, FilesError> + Send + 'static) -> Result<T, Failure> {
  background::run(work)
    .await
    .map_err(Failure::Interrupted)?
    .map_err(Failure::Files)
}

/// The files of `at`.
pub fn list(ctx: &mut Ctx, root: &Path, at: &Version, retry: u64) -> Shown<FileList> {
  let programs = Services::of(ctx).programs;
  let key = at.clone();
  let state = ctx
    .future((root.to_path_buf(), key.clone(), retry), move |(root, at, _)| {
      let programs = programs.clone();
      async move {
        let read = at.clone();
        let value = run(move || list_files(programs.runner(), &root, &read)).await?;
        Ok(Found { key: at, value })
      }
    })
    .state()
    .get();
  shown(&key, &state)
}

/// The file `path` at `at`.
pub fn file(ctx: &mut Ctx, root: &Path, (at, path): (&Version, &FilePath), retry: u64) -> Shown<FileView> {
  let programs = Services::of(ctx).programs;
  let key = (at.clone(), path.clone());
  let state = ctx
    .future((root.to_path_buf(), key.clone(), retry), move |(root, key, _)| {
      let programs = programs.clone();
      async move {
        let (at, path) = key.clone();
        let value = run(move || read_file(programs.runner(), &root, &at, &path)).await?;
        Ok(Found { key, value })
      }
    })
    .state()
    .get();
  shown(&key, &state)
}

/// The versions the page offers for `path` at `at`, opened from `run`.
pub fn choices(
  ctx: &mut Ctx,
  root: &Path,
  (at, run_id, path): (&Version, Option<RunId>, Option<&FilePath>),
  retry: u64,
) -> Shown<Arc<Versions>> {
  let programs = Services::of(ctx).programs;
  let key = (at.clone(), run_id, path.cloned());
  let state = ctx
    .future((root.to_path_buf(), key.clone(), retry), move |(root, key, _)| {
      let programs = programs.clone();
      async move {
        let (at, run_id, path) = key.clone();
        let value = run(move || versions(programs.runner(), &root, (&at, run_id), path.as_ref())).await?;
        Ok(Found {
          key,
          value: Arc::new(value),
        })
      }
    })
    .state()
    .get();
  shown(&key, &state)
}

/// The highlighted lines of `text`, the file at `path`, once a background thread made them; `None` before,
/// and for a file no grammar reads.
pub fn tokens(ctx: &mut Ctx, path: &FilePath, text: &Arc<FileText>) -> Option<Arc<Vec<LineTokens>>> {
  // The text is the same while its `Arc` is: a new read makes a new one.
  let key = (path.clone(), Arc::as_ptr(text) as usize);
  let text = text.clone();
  let state = ctx
    .future(key.clone(), move |key| {
      let text = text.clone();
      async move {
        let path = key.0.clone();
        let value = run(move || Ok(highlight_lines(path.as_str(), text.lines()).map(Arc::new))).await?;
        Ok(Found { key, value })
      }
    })
    .state()
    .get();
  match shown(&key, &state) {
    Shown::Read(tokens) => tokens,
    Shown::Reading | Shown::Failed(_) => None,
  }
}
