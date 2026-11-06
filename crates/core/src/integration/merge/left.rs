//! What a bring-in under way keeps beside the checkout, `<git dir>/orchester-bring-in-<run id>.json` in the checkout's
//! own git folder: what [`super::restore`] read before the fast-forward began. It is written before git begins and
//! removed once the merge is in or put back, so a bring-in that stopped part way — git stopped, or the app or the
//! runner ended — is found again: by the next merge into the checkout, refused while those paths do not hold the base
//! again ([`MergeError::NotPutBack`]); by [`put_back`], which puts them back; and when the app starts again
//! ([`clear_interrupted`]). A record whose checkout moved off its base since is dropped: what the checkout holds then
//! is the person's.

use std::{
  fs, io,
  path::{Path, PathBuf},
};

use super::{MergeError, MergeStep, TemporaryLeft, restore::Touched, temporary};
use crate::{
  command::CommandRunner,
  id::RunId,
  integration::{GitError, git::Git},
};

/// The start of a record's file name; the run's id and `.json` follow.
const PREFIX: &str = "orchester-bring-in-";
const SUFFIX: &str = ".json";

/// The record of the bring-in of `run`'s merge at `path`.
pub(super) struct Record {
  run: RunId,
  path: PathBuf,
}

impl Record {
  /// Keeps `touched`, what the bring-in of `run`'s merge changes, before git begins it.
  pub(super) fn save<R: CommandRunner>(git: &Git<'_, R>, run: RunId, touched: &Touched) -> Result<Self, MergeError> {
    let path = record_path(git, run)?;
    let text = serde_json::to_string(touched).expect("what a bring-in touches always serializes to JSON");
    fs::write(&path, text).map_err(|error| io_failure(&path, &error))?;
    Ok(Self { run, path })
  }

  /// Removes the record once nothing of its bring-in is left to put back.
  pub(super) fn remove(&self) -> Result<(), MergeError> {
    match fs::remove_file(&self.path) {
      Err(error) if error.kind() != io::ErrorKind::NotFound => Err(io_failure(&self.path, &error)),
      _ => Ok(()),
    }
  }

  fn read(&self) -> Result<Touched, MergeError> {
    let text = fs::read_to_string(&self.path).map_err(|error| io_failure(&self.path, &error))?;
    serde_json::from_str(&text).map_err(|error| {
      GitError::Unrecognized {
        expected: "the record of a bring-in",
        detail: format!("{}: {error}", self.path.display()),
      }
      .into()
    })
  }

  /// The paths its bring-in left changed; none, and the record removed, once they hold the base again or the
  /// checkout moved off the base.
  fn left<R: CommandRunner>(&self, git: &Git<'_, R>) -> Result<Vec<String>, MergeError> {
    let touched = self.read()?;
    let changed = if git.commit("HEAD")?.as_deref() == Some(touched.base()) {
      touched.still_changed(git)?
    } else {
      Vec::new()
    };
    if changed.is_empty() {
      self.remove()?;
    }
    Ok(changed)
  }
}

/// Refuses a merge into the checkout `git` runs in while a bring-in that stopped part way left files there changed.
pub(super) fn refuse_left<R: CommandRunner>(git: &Git<'_, R>) -> Result<(), MergeError> {
  for record in records(git)? {
    let files = record.left(git)?;
    if !files.is_empty() {
      return Err(MergeError::NotPutBack { run: record.run, files });
    }
  }
  Ok(())
}

/// The files the bring-in of `run`'s merge into the checkout at `repo` left changed when it stopped part way, with
/// the `git` program; empty when it left none, or nothing of it is left to put back.
pub fn left_by_bring_in(
  runner: &impl CommandRunner,
  (git, repo): (&Path, &Path),
  run: RunId,
) -> Result<Vec<String>, MergeError> {
  let git = Git::new(runner, git, repo);
  let record = Record {
    run,
    path: record_path(&git, run)?,
  };
  if !record.path.exists() {
    return Ok(Vec::new());
  }
  record.left(&git).map_err(|error| error.at(MergeStep::PutBack))
}

/// Puts back what the bring-in of `run`'s merge into the checkout at `repo` left changed, as when it stopped
/// ([`super::restore`]): never a file or index entry the person changed since. The paths that still do not hold the
/// base, as [`MergeError::LeftChanged`].
pub fn put_back(runner: &impl CommandRunner, (git, repo): (&Path, &Path), run: RunId) -> Result<(), MergeError> {
  let git = Git::new(runner, git, repo);
  let record = Record {
    run,
    path: record_path(&git, run)?,
  };
  let at = |error: MergeError| error.at(MergeStep::PutBack);
  if !record.path.exists() || record.left(&git).map_err(at)?.is_empty() {
    return Ok(());
  }
  let restored = record.read()?.restore(&git).map_err(at);
  let files = record.left(&git).map_err(at)?;
  match (restored, files.is_empty()) {
    (_, true) => Ok(()),
    (Err(MergeError::LeftChanged { detail, .. }), false) => Err(MergeError::LeftChanged { files, detail }),
    (Err(error), false) => Err(error),
    (Ok(()), false) => Err(MergeError::LeftChanged {
      files,
      detail: "they changed since the merge stopped".to_owned(),
    }),
  }
}

/// What a merge the app or the runner ended part way left, once its temporary worktree is removed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Interrupted {
  /// The files its bring-in left changed, to put back ([`put_back`]); empty when it left none.
  pub left_changed: Vec<String>,
  /// The temporary worktree, when it could not be removed.
  pub temporary: Option<TemporaryLeft>,
}

/// Clears what a merge of `run` into the checkout at `repo` that the app or the runner ended part way left: its
/// temporary worktree goes, and the files its bring-in left changed are named.
pub fn clear_interrupted(
  runner: &impl CommandRunner,
  (git, repo): (&Path, &Path),
  run: RunId,
) -> Result<Interrupted, MergeError> {
  let temporary = temporary::remove_left(&Git::new(runner, git, repo), run)?;
  let left_changed = left_by_bring_in(runner, (git, repo), run)?;
  Ok(Interrupted {
    left_changed,
    temporary,
  })
}

/// The records of bring-ins kept beside the checkout `git` runs in.
fn records<R: CommandRunner>(git: &Git<'_, R>) -> Result<Vec<Record>, MergeError> {
  let folder = git_dir(git)?;
  let entries = fs::read_dir(&folder).map_err(|error| io_failure(&folder, &error))?;
  let mut records = Vec::new();
  for entry in entries {
    let entry = entry.map_err(|error| io_failure(&folder, &error))?;
    let name = entry.file_name();
    let run = name
      .to_str()
      .and_then(|name| name.strip_prefix(PREFIX)?.strip_suffix(SUFFIX)?.parse::<RunId>().ok());
    if let Some(run) = run {
      records.push(Record {
        run,
        path: entry.path(),
      });
    }
  }
  Ok(records)
}

fn record_path<R: CommandRunner>(git: &Git<'_, R>, run: RunId) -> Result<PathBuf, GitError> {
  Ok(git_dir(git)?.join(format!("{PREFIX}{run}{SUFFIX}")))
}

/// The checkout's own git folder: a linked worktree's, not the one it shares.
fn git_dir<R: CommandRunner>(git: &Git<'_, R>) -> Result<PathBuf, GitError> {
  Ok(PathBuf::from(git.stdout(&["rev-parse", "--absolute-git-dir"])?))
}

fn io_failure(path: &Path, error: &io::Error) -> MergeError {
  GitError::Failed(format!("{}: {error}", path.display())).into()
}
