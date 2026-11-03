//! The repository's files as the @ menu offers them (ORC-316; `Files` in `08` `gOs7u`, `08b` `nUjD7` and `09` `Am5KT`
//! of `Mentions · redesign`): every file git tracks and every file it does not ignore, without tracked files that were
//! deleted and never in `.git` or `.orchester`, as paths relative to the repository's root — the project's checkout
//! in the Orchestrator, a run's worktree in its Conversation. Files the run changed (since the revision it started
//! from, or not tracked yet) lead and are marked; then the most recently modified. What is typed after the `@`
//! filters them fuzzily by path, ignoring ASCII case: a file whose name holds it first, then one whose path holds it,
//! then one whose path holds its characters in order; each group in that order.
//!
//! Listing reads git and every file's modification time and blocks: the UI reads it off the UI thread
//! ([`FileIndex::read_in`]). Filtering runs as the person types, in one pass over the index that keeps only the rows
//! shown ([`FileIndex::offer`]), so a repository of tens of thousands of files stays responsive.

use std::{
  collections::HashSet,
  fs,
  num::NonZeroUsize,
  ops::Range,
  path::{Path, PathBuf},
  sync::Arc,
  thread,
  time::SystemTime,
};

use orchester_core::{command::CommandRunner, id::RunId, integration::RunWork, revision::Revision, store::RunStore};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;

use super::{
  Mention,
  catalog::{Section, limit},
  writable_path,
};
use crate::{
  files::{FilePath, FilesError, git_output},
  messages::{self, Message},
  tasks::{changes::run_folder, detail::run_store_error},
};

/// The most threads that read the files' modification times.
const STAT_THREADS: usize = 8;

/// A file the menu offers: its path, whether the run changed it, and the bytes of its path that match what was typed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileChoice {
  pub path: FilePath,
  pub changed: bool,
  /// Where the path matches what was typed, in order; none while nothing is typed.
  pub letters: Arc<[Range<usize>]>,
}

impl FileChoice {
  pub fn mention(&self) -> Mention {
    Mention::File(self.path.clone())
  }

  /// Where the file's name starts in its path: after its folder and the `/` that ends it.
  pub fn name_at(&self) -> usize {
    name_at(self.path.as_str())
  }
}

/// A file as it was listed: its path, when it was last modified (`None` when that could not be read) and whether the
/// run changed it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
  pub path: FilePath,
  pub modified: Option<SystemTime>,
  pub changed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Indexed {
  path: FilePath,
  /// The path in ASCII lower case, which keeps every byte where it is.
  lower: Box<str>,
  changed: bool,
}

/// The files of one folder in the order the menu offers them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileIndex {
  files: Arc<[Indexed]>,
}

impl FileIndex {
  /// The index of `files`: the changed ones first, then the most recently modified, then by path.
  pub fn new(mut files: Vec<Listed>) -> Self {
    files.sort_by(|a, b| {
      b.changed
        .cmp(&a.changed)
        .then(b.modified.cmp(&a.modified))
        .then_with(|| a.path.cmp(&b.path))
    });
    let files = files
      .into_iter()
      .map(|file| Indexed {
        lower: file.path.as_str().to_ascii_lowercase().into(),
        path: file.path,
        changed: file.changed,
      })
      .collect();
    Self { files }
  }

  /// The files of the project at `repo_root`, or of `run`'s worktree there; a run that has no folder now (not
  /// started, or its worktree removed) lists the project's checkout, with nothing marked. Blocks on SQLite, git and
  /// the file system.
  pub fn read_in(programs: &Programs, repo_root: &Path, run: Option<RunId>) -> Result<Self, Message> {
    let (folder, base) = match run {
      Some(run) => run_place(repo_root, run)?,
      None => (repo_root.to_path_buf(), None),
    };
    Self::read(programs.runner(), &folder, base.as_ref()).map_err(|error| error.message())
  }

  /// The files of `folder`; with `base`, the revision a run started from, the files changed since then are marked.
  pub fn read(runner: &dyn CommandRunner, folder: &Path, base: Option<&Revision>) -> Result<Self, FilesError> {
    let deleted: HashSet<String> = listed(runner, folder, &["ls-files", "-z", "--deleted"])?
      .into_iter()
      .collect();
    let mut tracked = listed(runner, folder, &["ls-files", "-z", "--cached"])?;
    tracked.retain(|path| !deleted.contains(path));
    let untracked = listed(runner, folder, &["ls-files", "-z", "--others", "--exclude-standard"])?;
    let mut changed: HashSet<String> = match base {
      Some(base) => listed(runner, folder, &["diff", "--name-only", "-z", base.as_str(), "--"])?
        .into_iter()
        .collect(),
      None => HashSet::new(),
    };
    if base.is_some() {
      changed.extend(untracked.iter().cloned());
    }
    let paths: Vec<FilePath> = tracked
      .into_iter()
      .chain(untracked)
      .filter(|path| writable_path(path))
      .filter_map(|path| FilePath::new(&path).ok().filter(|file| file.as_str() == path))
      .collect();
    let modified = modified(folder, &paths);
    let files = paths
      .into_iter()
      .zip(modified)
      .map(|(path, modified)| Listed {
        changed: changed.contains(path.as_str()),
        path,
        modified,
      })
      .collect();
    Ok(Self::new(files))
  }

  pub fn len(&self) -> usize {
    self.files.len()
  }

  pub fn is_empty(&self) -> bool {
    self.files.is_empty()
  }

  /// The files the menu shows for `query`, typed after the `@`, and how many match in all.
  pub fn offer(&self, query: &str) -> Section<FileChoice> {
    let shown = limit(query);
    let query = query.to_ascii_lowercase();
    if query.is_empty() {
      return Section {
        shown: self.files.iter().take(shown).map(|file| choice(file, &query)).collect(),
        total: self.files.len(),
      };
    }
    let mut best: [Vec<&Indexed>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    let mut total = 0;
    for file in self.files.iter() {
      let Some(rank) = rank(&file.lower, &query) else {
        continue;
      };
      total += 1;
      if best[rank].len() < shown {
        best[rank].push(file);
      }
    }
    Section {
      shown: best
        .iter()
        .flatten()
        .take(shown)
        .map(|file| choice(file, &query))
        .collect(),
      total,
    }
  }
}

/// The folder `run` of the project at `repo_root` works in, and the revision it started from; the checkout when it
/// has no folder now.
fn run_place(repo_root: &Path, run: RunId) -> Result<(PathBuf, Option<Revision>), Message> {
  let db = ProjectDb::open(repo_root).map_err(|error| messages::store_error(&error))?;
  let record = db
    .run(run)
    .map_err(|error| messages::store_error(&run_store_error(error)))?;
  Ok(match (record, run_folder(&db, repo_root, run)) {
    (Some(record), Some(folder)) => (folder, Some(RunWork::of(&record).base)),
    _ => (repo_root.to_path_buf(), None),
  })
}

/// The paths `git <args>` lists, NUL-separated, in `folder`.
fn listed(runner: &dyn CommandRunner, folder: &Path, args: &[&str]) -> Result<Vec<String>, FilesError> {
  let output = git_output(runner, folder, args)?;
  Ok(
    output
      .split('\0')
      .filter(|path| !path.is_empty() && !path.ends_with('/'))
      .map(str::to_owned)
      .collect(),
  )
}

/// When each of `paths` in `folder` was last modified, read on a few threads at once.
fn modified(folder: &Path, paths: &[FilePath]) -> Vec<Option<SystemTime>> {
  let threads = thread::available_parallelism()
    .map_or(1, NonZeroUsize::get)
    .min(STAT_THREADS);
  let chunk = paths.len().div_ceil(threads).max(1);
  thread::scope(|scope| {
    let workers: Vec<_> = paths
      .chunks(chunk)
      .map(|chunk| {
        scope.spawn(move || {
          chunk
            .iter()
            .map(|path| {
              // A file that cannot be read now (removed since it was listed) is offered last, not left out.
              fs::symlink_metadata(folder.join(path.as_str()))
                .and_then(|metadata| metadata.modified())
                .ok()
            })
            .collect::<Vec<_>>()
        })
      })
      .collect();
    workers
      .into_iter()
      .flat_map(|worker| worker.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic)))
      .collect()
  })
}

/// How well `lower`, a path in lower case, answers `query`, lower is better: its name holds it, its path holds it, or
/// its path holds its characters in order; `None` when it does not.
fn rank(lower: &str, query: &str) -> Option<usize> {
  if lower[name_at(lower)..].contains(query) {
    Some(0)
  } else if lower.contains(query) {
    Some(1)
  } else {
    holds_in_order(lower, query).then_some(2)
  }
}

/// Whether `text` holds the characters of `query` in order; byte by byte for an ASCII query, which no byte of a wider
/// character can match.
fn holds_in_order(text: &str, query: &str) -> bool {
  if query.is_ascii() {
    let mut bytes = text.bytes();
    return query.bytes().all(|wanted| bytes.any(|byte| byte == wanted));
  }
  let mut characters = text.chars();
  query
    .chars()
    .all(|wanted| characters.any(|character| character == wanted))
}

/// `file` as the menu shows it for `query`, in lower case, with the letters that match it.
fn choice(file: &Indexed, query: &str) -> FileChoice {
  FileChoice {
    path: file.path.clone(),
    changed: file.changed,
    letters: letters(&file.lower, query).into(),
  }
}

/// Where `lower` matches `query`, as [`rank`] found it: the last place its name or its path holds it, else the first
/// letters in order.
fn letters(lower: &str, query: &str) -> Vec<Range<usize>> {
  if query.is_empty() {
    return Vec::new();
  }
  let name = name_at(lower);
  let whole = |at: usize| std::iter::once(at..at + query.len()).collect();
  if let Some(at) = lower[name..].rfind(query) {
    return whole(name + at);
  }
  match lower.rfind(query) {
    Some(at) => whole(at),
    None => in_order(lower, query).unwrap_or_default(),
  }
}

/// Where `text` holds the characters of `query` in order, the first of each, as runs of adjacent letters.
fn in_order(text: &str, query: &str) -> Option<Vec<Range<usize>>> {
  let mut wanted = query.chars().peekable();
  let mut letters: Vec<Range<usize>> = Vec::new();
  for (at, character) in text.char_indices() {
    let Some(&next) = wanted.peek() else {
      break;
    };
    if character != next {
      continue;
    }
    wanted.next();
    let end = at + character.len_utf8();
    match letters.last_mut() {
      Some(last) if last.end == at => last.end = end,
      _ => letters.push(at..end),
    }
  }
  wanted.peek().is_none().then_some(letters)
}

/// Where the name of the file at `path` starts.
fn name_at(path: &str) -> usize {
  path.rfind('/').map_or(0, |slash| slash + 1)
}
