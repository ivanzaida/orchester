//! *Resolve in editor* on a merge that conflicted: Orchester merges the run's base branch into the run's branch in
//! the run's own worktree, never in the person's checkout, leaves git's conflict markers there and opens the
//! worktree and its conflicted files in the person's editor ([`Editor`]). While that merge is in progress the task
//! page reads it ([`read`]); a file git could not merge by content waits for the person to keep one side
//! ([`decide`]); *Continue* commits the merge once every file is resolved ([`finish`]), after which the run's branch
//! holds the base branch and *Merge into main* is offered again; *Abort* puts the worktree back ([`abort`]).
//!
//! Only a run that succeeded, whose work was not merged and whose worktree is there and at its last checkpoint is
//! merged into, and only while its worktree holds no uncommitted work (`orchester_core::integration::resolve`). The
//! merge is recorded as the run's resolution before git begins it ([`ProjectDb::begin_resolution`]), in the same
//! transaction that checks the run ended: while it is recorded the runner neither returns nor follows the run up, and
//! the run is not merged; *Continue* and *Abort* end it, and a recorded resolution whose merge is gone from the
//! worktree (a person aborted it themselves) ends when the page reads the worktree again.

mod editor;

use std::path::Path;

use orchester_core::{
  command::CommandError,
  id::RunId,
  integration::{
    GitError,
    local::RebaseStore,
    resolve::{self, Resolution, ResolveError, Side, Started},
  },
  revision::Revision,
  run::Run,
  store::RunStore,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, ResolutionStart};

pub use self::editor::{EDITOR_VARIABLE, Editor, native};
use super::{
  changes::run_worktree,
  integration::{file_list, git_message},
};
use crate::messages::Message;

/// The git program, found on the search path.
const GIT: &str = "git";

/// How conflicted files are listed in a message.
const SEPARATOR: &str = ", ";

/// A run's worktree as returning and resolving read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Worktree {
  /// Its folder is there.
  pub present: bool,
  /// The merge in progress in it, whoever began it, with how each conflicted file stands.
  pub merge: Option<Resolution>,
}

/// `run`'s worktree under `repo_root` as it is now. With `idle` (no resolving of the run under way) a recorded
/// resolution whose merge is gone from the worktree ends: a person aborted or committed it themselves.
pub fn read(programs: &Programs, repo_root: &Path, (run, idle): (RunId, bool)) -> Result<Worktree, Message> {
  let read_at = Timestamp::now();
  let worktree = run_worktree(repo_root, run);
  if !worktree.is_dir() {
    // No merge waits in a worktree that is gone.
    open(repo_root)?.end_resolution(run).map_err(storage)?;
    return Ok(Worktree {
      present: false,
      merge: None,
    });
  }
  let merge = resolve::resolution(&programs.runner(), Path::new(GIT), &worktree).map_err(|error| problem(&error))?;
  if idle && merge.is_none() {
    // Only a resolution older than this read: one *Resolve in editor* begins meanwhile has not merged yet.
    open(repo_root)?.end_resolution_before(run, read_at).map_err(storage)?;
  }
  Ok(Worktree { present: true, merge })
}

/// Merges `run`'s base branch into its branch in its worktree: the files that conflict, or none when git merged
/// and committed it at once.
pub fn start(programs: &Programs, repo_root: &Path, run: RunId) -> Result<Started, Message> {
  let mut db = open(repo_root)?;
  let stored = stored(&db, run)?;
  let base = stored
    .base_branch()
    .ok_or_else(|| Message::new("runs.merge.failure.noBaseBranch"))?;
  let worktree = run_worktree(repo_root, run);
  if !worktree.is_dir() {
    return Err(Message::new("runs.control.worktreeRemoved"));
  }
  let expected = checkpoint_tip(&db, &stored)?;
  match db.begin_resolution(run, Timestamp::now()).map_err(storage)? {
    ResolutionStart::Begun => {}
    ResolutionStart::NotEnded => return Err(Message::new("tasks.resolve.notEnded")),
    ResolutionStart::Merged => return Err(Message::new("runs.control.merged")),
  }
  let started = resolve::start(
    &programs.git_work(),
    Path::new(GIT),
    &worktree,
    (base, stored.branch(), &expected),
  );
  // Only a merge that waits for the person keeps the resolution recorded.
  if !matches!(started, Ok(Started::Conflicts(_))) {
    db.end_resolution(run).map_err(storage)?;
  }
  started.map_err(|error| problem(&error))
}

/// Keeps `side`'s version of `path`, a file git could not merge by content, in `run`'s worktree.
pub fn decide(programs: &Programs, repo_root: &Path, (run, path, side): (RunId, &str, Side)) -> Result<(), Message> {
  let db = open(repo_root)?;
  settled(&stored(&db, run)?)?;
  resolve::decide(
    &programs.git_work(),
    Path::new(GIT),
    &run_worktree(repo_root, run),
    (path, side),
  )
  .map_err(|error| problem(&error))
}

/// Commits the merge in progress in `run`'s worktree once every conflicted file is resolved, and ends its resolution.
/// The run's branch then holds the base branch; the integration reads its work from there
/// (`local_integration::rebase`), which git proves from the merge commit.
pub fn finish(programs: &Programs, repo_root: &Path, run: RunId) -> Result<(), Message> {
  let mut db = open(repo_root)?;
  let stored = stored(&db, run)?;
  settled(&stored)?;
  let base = stored
    .base_branch()
    .ok_or_else(|| Message::new("runs.merge.failure.noBaseBranch"))?;
  let expected = checkpoint_tip(&db, &stored)?;
  resolve::finish(
    &programs.git_work(),
    Path::new(GIT),
    &run_worktree(repo_root, run),
    (base, &expected),
  )
  .map_err(|error| problem(&error))?;
  db.end_resolution(run).map_err(storage)
}

/// Aborts the merge in progress in `run`'s worktree and ends its resolution.
pub fn abort(programs: &Programs, repo_root: &Path, run: RunId) -> Result<(), Message> {
  let mut db = open(repo_root)?;
  settled(&stored(&db, run)?)?;
  resolve::abort(&programs.git_work(), Path::new(GIT), &run_worktree(repo_root, run))
    .map_err(|error| problem(&error))?;
  db.end_resolution(run).map_err(storage)
}

/// Why *Continue* waits: `files` still hold a conflict marker.
pub fn markers_left(files: &[String]) -> Message {
  Message::new("tasks.resolve.markersLeft").with("files", file_list(files, SEPARATOR))
}

/// Why *Continue* waits: `files` wait for the person to keep one side.
pub fn undecided(files: &[String]) -> Message {
  Message::new("tasks.resolve.undecided").with("files", file_list(files, SEPARATOR))
}

/// The commit `run`'s branch holds its last checkpoint at: the checkpoint, or the tip a recorded rebase or merge of
/// its base branch made of it; the run's base before its first checkpoint.
fn checkpoint_tip(db: &ProjectDb, run: &Run) -> Result<Revision, Message> {
  let last = run.last_checkpoint().map(|checkpoint| checkpoint.revision());
  let rebase = db.run_rebase(run.id()).map_err(storage)?;
  let tip = match &rebase {
    Some(rebase) => rebase.checkpoint_for(last),
    None => last,
  };
  Ok(tip.unwrap_or_else(|| run.base()).clone())
}

/// Refuses a change to the merge in `run`'s worktree while the run works there again.
fn settled(run: &Run) -> Result<(), Message> {
  if run.status().has_live_work() {
    Err(Message::new("tasks.resolve.notEnded"))
  } else {
    Ok(())
  }
}

fn open(repo_root: &Path) -> Result<ProjectDb, Message> {
  ProjectDb::open(repo_root).map_err(storage)
}

fn stored(db: &ProjectDb, run: RunId) -> Result<Run, Message> {
  db.run(run)
    .map_err(|error| {
      tracing::warn!(%error, %run, "cannot read the run to resolve its merge");
      Message::new("errors.storage")
    })?
    .ok_or_else(|| Message::new("errors.storage"))
}

fn storage(error: impl std::fmt::Display) -> Message {
  tracing::warn!(%error, "the project database failed while resolving a run's merge");
  Message::new("errors.storage")
}

/// What a person reads when resolving did not go on.
fn problem(error: &ResolveError) -> Message {
  let files = |key, files: &[String]| Message::new(key).with("files", file_list(files, SEPARATOR));
  match error {
    ResolveError::Git(GitError::Run(CommandError::NotFound)) => Message::new("runs.changes.failure.gitMissing"),
    ResolveError::Git(error) => git_message(error),
    ResolveError::Uncommitted(changed) => files("tasks.resolve.uncommitted", changed),
    ResolveError::NotOnBranch => Message::new("tasks.resolve.notOnBranch"),
    ResolveError::BranchMoved => Message::new("tasks.resolve.branchMoved"),
    ResolveError::NotTheBase => Message::new("tasks.resolve.notTheBase"),
    ResolveError::InMerge => Message::new("tasks.resolve.inMerge"),
    ResolveError::NotInMerge => Message::new("tasks.resolve.notInMerge"),
    ResolveError::MarkersLeft(left) => markers_left(left),
    ResolveError::Undecided(left) => undecided(left),
    ResolveError::NotUnmerged(path) => Message::new("tasks.resolve.notUnmerged").with("file", path.clone()),
  }
}
