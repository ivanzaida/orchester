//! *Resolve in editor* on a merge that conflicted: Orchester merges the run's base branch into the run's branch in
//! the run's own worktree, never in the person's checkout, leaves git's conflict markers there and opens the
//! worktree and its conflicted files in the person's editor ([`Editor`]). While that merge is in progress the task
//! page reads it ([`read`]); *Continue* commits it once no marker is left ([`finish`]), after which the run's branch
//! holds the base branch and *Merge into main* is offered again; *Abort* puts the worktree back ([`abort`]). Only a
//! run that ended, whose work was not merged and whose worktree is there is merged into, and only while its
//! worktree holds no uncommitted work (`orchester_core::integration::resolve`).

use std::{
  env, io,
  path::{Path, PathBuf},
};

use orchester_core::{
  command::CommandError,
  connection::executable_candidates,
  id::RunId,
  integration::{
    GitError,
    local::{RebaseStore, RunRebase},
    resolve::{self, MergedIn, Resolution, ResolveError, Started},
  },
  run::Run,
  store::{MergeStore, RunStore},
  time::Timestamp,
};
use orchester_exec::{external, programs::Programs, search_path::search_path};
use orchester_store::ProjectDb;

use super::{
  changes::run_worktree,
  detail::run_store_error,
  integration::{file_list, git_message},
};
use crate::{files::Platform, messages::Message};

/// The variable that names the editor *Resolve in editor* starts, a program given the worktree and the conflicted
/// files; a QA copy names a stand-in so that no editor opens on the person's desktop.
pub const EDITOR_VARIABLE: &str = "ORCHESTER_EDITOR";

/// The git program, found on the search path.
const GIT: &str = "git";

/// How conflicted files are listed in a message.
const SEPARATOR: &str = ", ";

/// The editor the worktree opens in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Editor {
  /// The program [`EDITOR_VARIABLE`] names.
  Named(PathBuf),
  /// VS Code's `code`, found on the search path.
  VsCode(PathBuf),
  /// The system's default program for folders (Explorer, Finder): the worktree opens, its files do not.
  SystemFolders,
}

impl Editor {
  /// The editor this computer has: the one [`EDITOR_VARIABLE`] names, else VS Code when its `code` is on the search
  /// path, else the system's default for folders; `None` where the platform has none.
  pub fn find() -> Option<Self> {
    if let Some(named) = env::var_os(EDITOR_VARIABLE).filter(|named| !named.is_empty()) {
      return Some(Self::Named(PathBuf::from(named)));
    }
    let path = search_path();
    if let Some(code) = executable_candidates("code", path.as_deref())
      .into_iter()
      .find(|candidate| candidate.is_file())
    {
      return Some(Self::VsCode(code));
    }
    Platform::current().map(|_| Self::SystemFolders)
  }

  /// Whether automation driving the window may start it: only an editor a person named for this purpose.
  pub const fn named(&self) -> bool {
    matches!(self, Self::Named(_))
  }

  /// Opens `worktree` and its `files` (paths from its root); the system's folder program opens the folder alone.
  pub fn open(&self, worktree: &Path, files: &[String]) -> io::Result<()> {
    let program = match self {
      Self::Named(program) | Self::VsCode(program) => program,
      Self::SystemFolders => return external::open_path(worktree),
    };
    let files: Vec<PathBuf> = files.iter().map(|file| worktree.join(file)).collect();
    let args: Vec<&Path> = std::iter::once(worktree)
      .chain(files.iter().map(PathBuf::as_path))
      .collect();
    external::start_editor(program, &args)
  }
}

/// The merge in progress in `run`'s worktree under `repo_root`, if any; `None` too once the worktree is gone.
pub fn read(programs: &Programs, repo_root: &Path, run: RunId) -> Result<Option<Resolution>, Message> {
  let worktree = run_worktree(repo_root, run);
  if !worktree.is_dir() {
    return Ok(None);
  }
  resolve::resolution(&programs.runner(), Path::new(GIT), &worktree).map_err(|error| problem(&error))
}

/// Merges `run`'s base branch into its branch in its worktree: the files that conflict, or none when git merged
/// and committed it at once.
pub fn start(programs: &Programs, repo_root: &Path, run: RunId) -> Result<Started, Message> {
  let db = ProjectDb::open(repo_root).map_err(|_| Message::new("errors.storage"))?;
  let stored = db
    .run(run)
    .map_err(run_store_error)
    .map_err(|_| Message::new("errors.storage"))?
    .ok_or_else(|| Message::new("errors.storage"))?;
  if stored.status().has_live_work() || !stored.can_return_ended() {
    return Err(Message::new("tasks.resolve.notEnded"));
  }
  if db.run_merge(run).map_err(|_| Message::new("errors.storage"))?.is_some() {
    return Err(Message::new("runs.control.merged"));
  }
  let base = stored
    .base_branch()
    .ok_or_else(|| Message::new("runs.merge.failure.noBaseBranch"))?;
  let worktree = run_worktree(repo_root, run);
  if !worktree.is_dir() {
    return Err(Message::new("runs.control.worktreeRemoved"));
  }
  let started = resolve::start(&programs.git_work(), Path::new(GIT), &worktree, (base, stored.branch()))
    .map_err(|error| problem(&error))?;
  if let Started::Merged(merged) = &started {
    record(repo_root, &stored, merged)?;
  }
  Ok(started)
}

/// Commits the merge in progress in `run`'s worktree once no conflicted file holds a marker.
pub fn finish(programs: &Programs, repo_root: &Path, run: RunId) -> Result<(), Message> {
  let merged = resolve::finish(&programs.git_work(), Path::new(GIT), &run_worktree(repo_root, run))
    .map_err(|error| problem(&error))?;
  let db = ProjectDb::open(repo_root).map_err(|_| Message::new("errors.storage"))?;
  let stored = db
    .run(run)
    .map_err(|_| Message::new("errors.storage"))?
    .ok_or_else(|| Message::new("errors.storage"))?;
  record(repo_root, &stored, &merged)
}

/// Records that the base branch was merged into `run`'s branch as `merged`: its own commits sit on that commit now,
/// so merging it into the base branch reads its work from there (`orchester_core::integration::local::RunRebase`).
fn record(repo_root: &Path, run: &Run, merged: &MergedIn) -> Result<(), Message> {
  let last = run.last_checkpoint().map(|checkpoint| checkpoint.revision());
  let rebase = RunRebase::merged_in(
    run.id(),
    (merged.base.clone(), merged.tip.clone()),
    last,
    Timestamp::now(),
  );
  ProjectDb::open(repo_root)
    .and_then(|mut db| db.save_run_rebase(&rebase))
    .map_err(|error| {
      tracing::warn!(%error, run = %run.id(), "cannot record the merge into the run's branch");
      Message::new("errors.storage")
    })
}

/// Aborts the merge in progress in `run`'s worktree.
pub fn abort(programs: &Programs, repo_root: &Path, run: RunId) -> Result<(), Message> {
  resolve::abort(&programs.git_work(), Path::new(GIT), &run_worktree(repo_root, run)).map_err(|error| problem(&error))
}

/// Why *Continue* waits: `files` still hold a conflict marker.
pub fn markers_left(files: &[String]) -> Message {
  Message::new("tasks.resolve.markersLeft").with("files", file_list(files, SEPARATOR))
}

/// What a person reads when resolving did not go on.
fn problem(error: &ResolveError) -> Message {
  let files = |key, files: &[String]| Message::new(key).with("files", file_list(files, SEPARATOR));
  match error {
    ResolveError::Git(GitError::Run(CommandError::NotFound)) => Message::new("runs.changes.failure.gitMissing"),
    ResolveError::Git(error) => git_message(error),
    ResolveError::Uncommitted(changed) => files("tasks.resolve.uncommitted", changed),
    ResolveError::NotOnBranch => Message::new("tasks.resolve.notOnBranch"),
    ResolveError::InMerge => Message::new("tasks.resolve.inMerge"),
    ResolveError::NotInMerge => Message::new("tasks.resolve.notInMerge"),
    ResolveError::MarkersLeft(left) => markers_left(left),
    ResolveError::Unreadable { path, .. } => Message::new("tasks.resolve.unreadable").with("file", path.clone()),
  }
}
