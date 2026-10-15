//! What the code viewer reads off the UI thread: the version's files, the file, the versions it has, and
//! the file's highlighted lines. A read's result names what it was read for, since a future keeps its last
//! result while it reads again; a result for other arguments is not shown. The results' DevTools entries
//! are empty, so a file's text never reaches the DevTools tree.

use std::{
  path::Path,
  sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
  },
};

use lurq::app::{component::DevtoolsInspectable, ctx::Ctx};
use orchester_core::id::RunId;
use orchester_workspace::{
  background,
  files::{FileList, FilePath, FileText, FileView, FilesError, Version, Versions, list_files, read_file, versions},
  highlight::{LineTokens, highlight_lines},
  messages::Message,
};
use parking_lot::Mutex;

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

/// A read's value with what it was read for, and which read it was.
#[derive(Clone, PartialEq)]
pub struct Found<K, T> {
  pub key: K,
  pub value: T,
  serial: u64,
}

impl<K, T> Found<K, T> {
  fn new(key: K, value: T) -> Self {
    static READS: AtomicU64 = AtomicU64::new(0);
    Self {
      key,
      value,
      serial: READS.fetch_add(1, Ordering::Relaxed),
    }
  }
}

/// What makes the page read again: Try again, the runner's changes of runs and transcripts while an agent
/// works in the version shown, and the project's integration operations ([`orchester_workspace::files::Follow`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Wake {
  pub retry: u64,
  pub runs: u64,
  pub jobs: u64,
}

/// Keeps the value shown when a read again finds what was there: the page renders nothing new for it, and
/// the file keeps its colors, its selection and where it is scrolled to.
pub struct Steady<T>(Mutex<Option<(u64, T)>>);

impl<T> Default for Steady<T> {
  fn default() -> Self {
    Self(Mutex::new(None))
  }
}

impl<T: Clone + PartialEq> Steady<T> {
  /// `read`, or the value kept when it holds the same; each read is compared once.
  fn keep(&self, read: Shown<(u64, T)>) -> Shown<T> {
    let (serial, value) = match read {
      Shown::Read(read) => read,
      Shown::Reading => return Shown::Reading,
      Shown::Failed(failure) => return Shown::Failed(failure),
    };
    let mut kept = self.0.lock();
    match kept.as_mut() {
      Some((seen, shown)) if *seen == serial => Shown::Read(shown.clone()),
      Some((seen, shown)) if *shown == value => {
        *seen = serial;
        Shown::Read(shown.clone())
      }
      _ => {
        *kept = Some((serial, value.clone()));
        Shown::Read(value)
      }
    }
  }
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

/// The result of a future for `key`, with which read it was: its value when it was read for `key` (the last
/// one while it is read again), else its failure, else that it is being read.
fn shown<K: PartialEq, T: Clone>(
  key: &K,
  state: &lurq::app::ctx::FutureState<Found<K, T>, Failure>,
) -> Shown<(u64, T)> {
  if let Some(found) = state.data.as_ref()
    && found.key == *key
    && !state.is_rejected()
  {
    return Shown::Read((found.serial, found.value.clone()));
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
pub fn list(ctx: &mut Ctx, root: &Path, at: &Version, (wake, kept): (Wake, &Steady<FileList>)) -> Shown<FileList> {
  let programs = Services::of(ctx).programs;
  let key = at.clone();
  let state = ctx
    .future((root.to_path_buf(), key.clone(), wake), move |(root, at, _)| {
      let programs = programs.clone();
      async move {
        let read = at.clone();
        let value = run(move || list_files(programs.runner(), &root, &read)).await?;
        Ok(Found::new(at, value))
      }
    })
    .state()
    .get();
  kept.keep(shown(&key, &state))
}

/// The file `path` at `at`.
pub fn file(
  ctx: &mut Ctx,
  root: &Path,
  (at, path): (&Version, &FilePath),
  (wake, kept): (Wake, &Steady<FileView>),
) -> Shown<FileView> {
  let programs = Services::of(ctx).programs;
  let key = (at.clone(), path.clone());
  let state = ctx
    .future((root.to_path_buf(), key.clone(), wake), move |(root, key, _)| {
      let programs = programs.clone();
      async move {
        let (at, path) = key.clone();
        let value = run(move || read_file(programs.runner(), &root, &at, &path)).await?;
        Ok(Found::new(key, value))
      }
    })
    .state()
    .get();
  kept.keep(shown(&key, &state))
}

/// The versions the page offers for `path` at `at`, opened from `run`.
pub fn choices(
  ctx: &mut Ctx,
  root: &Path,
  (at, run_id, path): (&Version, Option<RunId>, Option<&FilePath>),
  wake: Wake,
) -> Shown<Arc<Versions>> {
  let programs = Services::of(ctx).programs;
  let key = (at.clone(), run_id, path.cloned());
  let state = ctx
    .future((root.to_path_buf(), key.clone(), wake), move |(root, key, _)| {
      let programs = programs.clone();
      async move {
        let (at, run_id, path) = key.clone();
        let value = run(move || versions(programs.runner(), &root, (&at, run_id), path.as_ref())).await?;
        Ok(Found::new(key, Arc::new(value)))
      }
    })
    .state()
    .get();
  match shown(&key, &state) {
    Shown::Read((_, versions)) => Shown::Read(versions),
    Shown::Reading => Shown::Reading,
    Shown::Failed(failure) => Shown::Failed(failure),
  }
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
        Ok(Found::new(key, value))
      }
    })
    .state()
    .get();
  match shown(&key, &state) {
    Shown::Read((_, tokens)) => tokens,
    Shown::Reading | Shown::Failed(_) => None,
  }
}
