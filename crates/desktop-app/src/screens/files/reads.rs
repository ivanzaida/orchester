//! What the code viewer reads off the UI thread: the version's files, the file, the versions it has, and
//! the file's highlighted lines. A read's result names what it was read for, since a future keeps its last
//! result while it reads again; a result for other arguments is not shown. The results' DevTools entries
//! are empty, so a file's text never reaches the DevTools tree.

use std::{
  path::Path,
  sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
  },
  thread,
  time::Duration,
};

use lurq::app::{component::DevtoolsInspectable, ctx::Ctx};
use orchester_core::{command::CommandRunner, id::RunId};
use orchester_workspace::{
  background,
  files::{
    Digest, FileList, FilePath, FileText, FileView, FilesError, Version, Versions, list_files, read_file, versions,
  },
  highlight::{LineTokens, highlight_lines},
  messages::Message,
};
use parking_lot::Mutex;

use super::shown_lines::cut_lines;
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

  /// A failure that passes: what was there could not be read for a moment (another program holds it, or
  /// replaces it), so a read again that meets it keeps what it read last.
  pub const fn passes(&self) -> bool {
    matches!(self, Self::Files(FilesError::Unreadable { .. }))
  }
}

/// A read's value with what it was read for, and a digest of what it holds.
#[derive(Clone, PartialEq)]
pub struct Found<K, T> {
  pub key: K,
  pub value: T,
  digest: Option<Digest>,
}

impl<K, T> Found<K, T> {
  /// `value` read for `key`; `digest`, where the page keeps what it shows ([`Steady`]), tells a read again that
  /// found the same, worked out off the UI thread.
  fn new(key: K, value: T, digest: Option<Digest>) -> Self {
    Self { key, value, digest }
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

impl Wake {
  /// What wakes the version menu: not the runner's changes, which change no version, only what does.
  const fn versions(self) -> Self {
    Self { runs: 0, ..self }
  }
}

/// An `Arc` compared by which allocation it is: the same text while its `Arc` is, since a read makes a new one.
#[derive(Clone)]
pub struct Same<T>(pub Arc<T>);

impl<T> PartialEq for Same<T> {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl<T> DevtoolsInspectable for Same<T> {}

/// Keeps the value shown when a read again finds what was there (the same digest): the page renders nothing new
/// for it, and the file keeps its colors, its selection and where it is scrolled to.
pub struct Steady<T>(Mutex<Option<(Option<Digest>, T)>>);

impl<T> Default for Steady<T> {
  fn default() -> Self {
    Self(Mutex::new(None))
  }
}

impl<T: Clone> Steady<T> {
  /// `read`, or the value kept when it holds the same.
  fn keep(&self, read: Shown<(Option<Digest>, T)>) -> Shown<T> {
    let (digest, value) = match read {
      Shown::Read(read) => read,
      Shown::Reading => return Shown::Reading,
      Shown::Failed(failure) => return Shown::Failed(failure),
    };
    let mut kept = self.0.lock();
    match kept.as_ref() {
      Some((shown, value)) if *shown == digest => Shown::Read(value.clone()),
      _ => {
        *kept = Some((digest, value.clone()));
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
/// one while it is read again, and when reading it again failed for a moment), else its failure, else that
/// it is being read.
fn shown<K: PartialEq, T: Clone>(
  key: &K,
  state: &lurq::app::ctx::FutureState<Found<K, T>, Failure>,
) -> Shown<(Option<Digest>, T)> {
  if let Some(found) = state.data.as_ref()
    && found.key == *key
    && (!state.is_rejected() || state.error.as_ref().is_some_and(Failure::passes))
  {
    return Shown::Read((found.digest, found.value.clone()));
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
        let digest = value.digest();
        Ok(Found::new(at, value, Some(digest)))
      }
    })
    .state()
    .get();
  kept.keep(shown(&key, &state))
}

/// How often the page read its file, and how many reads again in a row met a file that could not be read for a
/// moment and kept the text read before.
#[derive(Default)]
pub struct FileReads {
  started: AtomicU64,
  kept_over: AtomicU32,
}

impl FileReads {
  /// The reads of the file the page started.
  pub fn started(&self) -> u64 {
    self.started.load(Ordering::Relaxed)
  }

  fn settled<T>(&self, read: &Result<T, Failure>) {
    match read {
      Ok(_) => self.kept_over.store(0, Ordering::Relaxed),
      Err(failure) if failure.passes() => {
        self.kept_over.fetch_add(1, Ordering::Relaxed);
      }
      Err(_) => {}
    }
  }
}

/// Reads again in a row that met a file that could not be read before the text kept may be out of date: one is
/// a moment's (a save by rename, another program's lock), two are not.
const KEPT_OVER: u32 = 2;

/// What the page reads its file with: what wakes the read, the value kept, the reads counted, and whether the
/// page follows the runner's changes.
pub type FileFollowing<'a> = (Wake, &'a Steady<FileView>, &'a Arc<FileReads>, bool);

/// The file `path` at `at`, and why the text shown may be out of date: it is the text read before, kept over
/// [`KEPT_OVER`] reads again in a row that could not read the file, or over one while the page no longer follows
/// the runner's changes, since nothing reads it again soon.
pub fn file(
  ctx: &mut Ctx,
  root: &Path,
  (at, path): (&Version, &FilePath),
  (wake, kept, reads, following): FileFollowing<'_>,
) -> (Shown<FileView>, Option<Failure>) {
  let programs = Services::of(ctx).programs;
  let key = (at.clone(), path.clone());
  let counted = reads.clone();
  let state = ctx
    .future((root.to_path_buf(), key.clone(), wake), move |(root, key, _)| {
      let programs = programs.clone();
      let reads = counted.clone();
      reads.started.fetch_add(1, Ordering::Relaxed);
      async move {
        let (at, path) = key.clone();
        let read = run(move || read_steadily(programs.runner(), &root, (&at, &path))).await;
        reads.settled(&read);
        let value = read?;
        let digest = value.digest();
        Ok(Found::new(key, value, Some(digest)))
      }
    })
    .state()
    .get();
  let shown = kept.keep(shown(&key, &state));
  let kept_over = reads.kept_over.load(Ordering::Relaxed);
  let stale = match &shown {
    Shown::Read(_) if state.is_rejected() && (kept_over >= KEPT_OVER || (!following && kept_over > 0)) => state.error,
    _ => None,
  };
  (shown, stale)
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
    .future(
      (root.to_path_buf(), key.clone(), wake.versions()),
      move |(root, key, _)| {
        let programs = programs.clone();
        async move {
          let (at, run_id, path) = key.clone();
          let value = run(move || versions(programs.runner(), &root, (&at, run_id), path.as_ref())).await?;
          Ok(Found::new(key, Arc::new(value), None))
        }
      },
    )
    .state()
    .get();
  match shown(&key, &state) {
    Shown::Read((_, versions)) => Shown::Read(versions),
    Shown::Reading => Shown::Reading,
    Shown::Failed(failure) => Shown::Failed(failure),
  }
}

/// The highlighted lines of `text`, the file at `path`, once a background thread made them; `None` before,
/// and for a file no grammar reads. A pass for an older text stops once a newer one asks: `superseded` holds
/// the flag of the pass under way.
pub fn tokens(
  ctx: &mut Ctx,
  (path, text): (&FilePath, &Arc<FileText>),
  superseded: &Arc<Mutex<Arc<AtomicBool>>>,
) -> Option<Arc<Vec<LineTokens>>> {
  let key = (path.clone(), Same(text.clone()));
  let superseded = superseded.clone();
  let state = ctx
    .future(key.clone(), move |key| {
      let wanted = Arc::new(AtomicBool::new(true));
      let earlier = std::mem::replace(&mut *superseded.lock(), wanted.clone());
      earlier.store(false, Ordering::Relaxed);
      async move {
        let (path, text) = (key.0.clone(), key.1.0.clone());
        let still = move || wanted.load(Ordering::Relaxed);
        let value =
          run(move || Ok(highlight_lines(path.as_str(), text.lines(), &still).map(|lines| Arc::new(cut_lines(lines)))))
            .await?;
        Ok(Found::new(key, value, None))
      }
    })
    .state()
    .get();
  match shown(&key, &state) {
    Shown::Read((_, tokens)) => tokens,
    Shown::Reading | Shown::Failed(_) => None,
  }
}

/// How long a read waits before it reads once more a file it found missing or unreadable: long enough for an
/// editor's or an agent's save by rename to finish.
const SETTLE: Duration = Duration::from_millis(50);

/// The file `path` at `at`, read once more after [`SETTLE`] when it was missing or unreadable, so a save by
/// rename does not show as a missing file.
fn read_steadily(
  runner: &dyn CommandRunner,
  root: &Path,
  (at, path): (&Version, &FilePath),
) -> Result<FileView, FilesError> {
  match read_file(runner, root, at, path) {
    Err(FilesError::Missing(_) | FilesError::Unreadable { .. }) => {
      thread::sleep(SETTLE);
      read_file(runner, root, at, path)
    }
    read => read,
  }
}
