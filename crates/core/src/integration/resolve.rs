//! Resolving by hand what conflicted when a run was merged into its base branch: the base branch is merged into
//! the run's branch in the run's own worktree — never in the person's checkout — and git leaves its conflict
//! markers there for the person to resolve in their editor ([`start`]). While that merge is in progress the
//! worktree tells how each conflicted file stands ([`resolution`]): the index is the truth, so a file the person (or
//! their editor) marked resolved is resolved whatever its text holds; a file still unmerged is resolved once its
//! text holds no marker, and a file git could not merge by content (a binary file, or one a side deleted) waits for
//! the person to keep one side ([`decide`]). [`finish`] commits the merge once every file is resolved, after which
//! the run's branch holds the base branch and merges into it without conflicts; [`abort`] puts the worktree back. A
//! worktree with uncommitted work, or whose branch moved past the run's last checkpoint, is not merged into.

use std::{collections::BTreeMap, fs, io::Read as _, path::Path};

use thiserror::Error;

use super::{
  GitError,
  git::{Git, message},
};
use crate::{command::CommandRunner, revision::Revision, run::BranchName};

/// What a line of a file that still holds a conflict starts with: git's markers around and inside a conflict
/// (`diff3` style included). A line of `=======` alone is not looked for: Markdown underlines a heading with it.
const MARKERS: [&str; 3] = ["<<<<<<<", ">>>>>>>", "|||||||"];

/// Bytes looked at for a NUL, git's own test for a binary file.
const BINARY_PROBE: u64 = 8000;

/// The index stage of the run's side of a merge into its worktree (`--ours`), and of the base branch's (`--theirs`).
const RUN_STAGE: u8 = 2;
const BASE_STAGE: u8 = 3;

/// A side of the merge a person keeps for a file git could not merge by content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
  /// The run's version (`--ours`): its branch is checked out in the worktree.
  Run,
  /// The base branch's version (`--theirs`).
  Base,
}

/// Why a file waits for the person to keep one side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Undecided {
  /// Git could not merge its content: a binary file, or text git keeps as binary (UTF-16, say).
  Unmergeable,
  /// The base branch deleted it, the run changed it.
  DeletedByBase,
  /// The run deleted it, the base branch changed it.
  DeletedByRun,
}

/// How a file that conflicted stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileState {
  /// Still unmerged, and its text holds git's markers.
  Markers,
  /// Still unmerged, and the person keeps one side.
  Undecided(Undecided),
  /// Marked resolved in the index, or still unmerged with text that holds no marker any more.
  Resolved,
}

/// A file that conflicted in the merge, as `git` names it from the worktree's root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConflictFile {
  pub path: String,
  pub state: FileState,
}

/// A merge in progress in a run's worktree: every file that conflicted in it, still unmerged ones first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolution {
  pub files: Vec<ConflictFile>,
}

impl Resolution {
  /// The files still holding a marker.
  pub fn with_markers(&self) -> Vec<String> {
    self.paths(|state| state == FileState::Markers)
  }

  /// The files waiting for the person to keep one side.
  pub fn undecided(&self) -> Vec<String> {
    self.paths(|state| matches!(state, FileState::Undecided(_)))
  }

  fn paths(&self, chosen: impl Fn(FileState) -> bool) -> Vec<String> {
    self
      .files
      .iter()
      .filter(|file| chosen(file.state))
      .map(|file| file.path.clone())
      .collect()
  }
}

/// The merge of the base branch committed in the run's worktree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergedIn {
  /// The commit of the base branch that was merged in: the run's own commits sit on it now.
  pub base: Revision,
  /// The merge commit, the run's branch's tip.
  pub tip: Revision,
}

/// How merging the base branch into the run's branch went.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Started {
  /// Git merged without a conflict and committed the merge.
  Merged(MergedIn),
  /// These files conflict; the merge waits in the worktree with git's markers in them.
  Conflicts(Vec<String>),
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ResolveError {
  #[error(transparent)]
  Git(#[from] GitError),
  /// The worktree has changes nobody committed: they are the agent's, and merging would mix them in.
  #[error("the worktree has uncommitted changes: {}", .0.join(", "))]
  Uncommitted(Vec<String>),
  /// The worktree is not on the run's branch, so merging there would not change the run's work.
  #[error("the worktree is not on the run's branch")]
  NotOnBranch,
  /// The run's branch is not at its last checkpoint: commits Orchester did not record would be merged with.
  #[error("the run's branch moved past its last checkpoint")]
  BranchMoved,
  /// The merge in progress does not merge the base branch: someone else's merge is in the worktree.
  #[error("the merge in progress does not merge the base branch")]
  NotTheBase,
  /// A merge is in progress in the worktree already.
  #[error("a merge is in progress in the worktree already")]
  InMerge,
  /// No merge is in progress in the worktree.
  #[error("no merge is in progress in the worktree")]
  NotInMerge,
  /// These conflicted files still hold conflict markers.
  #[error("conflict markers are left in {}", .0.join(", "))]
  MarkersLeft(Vec<String>),
  /// These files wait for the person to keep one side.
  #[error("{} wait for a side to be kept", .0.join(", "))]
  Undecided(Vec<String>),
  /// The file is not one the merge left unmerged.
  #[error("{0} is not unmerged")]
  NotUnmerged(String),
  /// A conflicted file could not be read.
  #[error("{path} cannot be read: {detail}")]
  Unreadable { path: String, detail: String },
}

/// Whether a merge is in progress in `worktree`.
pub fn in_merge(runner: &impl CommandRunner, git: &Path, worktree: &Path) -> Result<bool, GitError> {
  merging(&Git::new(runner, git, worktree))
}

/// The merge in progress in `worktree`, if any, with how each conflicted file stands.
pub fn resolution(
  runner: &impl CommandRunner,
  git: &Path,
  worktree: &Path,
) -> Result<Option<Resolution>, ResolveError> {
  let in_worktree = Git::new(runner, git, worktree);
  if !merging(&in_worktree)? {
    return Ok(None);
  }
  let mut files = Vec::new();
  for (path, stages) in staged(&in_worktree, &["ls-files", "--unmerged", "-z"])? {
    let state = unmerged_state(worktree, &path, &stages)?;
    files.push(ConflictFile { path, state });
  }
  for (path, _) in staged(&in_worktree, &["ls-files", "--resolve-undo", "-z"])? {
    if !files.iter().any(|file| file.path == path) {
      files.push(ConflictFile {
        path,
        state: FileState::Resolved,
      });
    }
  }
  Ok(Some(Resolution { files }))
}

/// Merges `base` into `branch`, the run's branch checked out in its `worktree` at `expected` (its last checkpoint),
/// leaving the conflicts there for a person to resolve.
pub fn start(
  runner: &impl CommandRunner,
  git: &Path,
  worktree: &Path,
  (base, branch, expected): (&BranchName, &BranchName, &Revision),
) -> Result<Started, ResolveError> {
  let git = Git::new(runner, git, worktree);
  if merging(&git)? {
    return Err(ResolveError::InMerge);
  }
  if git.checked_out_branch()?.as_ref() != Some(branch) {
    return Err(ResolveError::NotOnBranch);
  }
  if git.stdout(&["rev-parse", "--verify", "HEAD"])? != expected.as_str() {
    return Err(ResolveError::BranchMoved);
  }
  let uncommitted = git.uncommitted()?;
  if !uncommitted.is_empty() {
    return Err(ResolveError::Uncommitted(uncommitted));
  }
  let title = format!("Merge {} into {}", base.as_str(), branch.as_str());
  let base = format!("refs/heads/{}", base.as_str());
  let output = git.output(&["merge", "--no-ff", "--no-edit", "-m", &title, "--end-of-options", &base])?;
  if output.success() {
    return Ok(Started::Merged(merged_in(&git)?));
  }
  let conflicts: Vec<String> = staged(&git, &["ls-files", "--unmerged", "-z"])?.into_keys().collect();
  if conflicts.is_empty() || !merging(&git)? {
    return Err(GitError::Failed(message(&output)).into());
  }
  Ok(Started::Conflicts(conflicts))
}

/// Keeps `side`'s version of `path`, a file git could not merge by content, and marks it resolved; a side that
/// deleted the file deletes it.
pub fn decide(
  runner: &impl CommandRunner,
  git: &Path,
  worktree: &Path,
  (path, side): (&str, Side),
) -> Result<(), ResolveError> {
  let git = Git::new(runner, git, worktree);
  if !merging(&git)? {
    return Err(ResolveError::NotInMerge);
  }
  let stages = staged(&git, &["ls-files", "--unmerged", "-z", "--", path])?
    .remove(path)
    .ok_or_else(|| ResolveError::NotUnmerged(path.to_owned()))?;
  let (stage, option) = match side {
    Side::Run => (RUN_STAGE, "--ours"),
    Side::Base => (BASE_STAGE, "--theirs"),
  };
  if stages.contains(&stage) {
    git.stdout(&["checkout", option, "--", path])?;
    git.stdout(&["add", "--", path])?;
  } else {
    git.stdout(&["rm", "--quiet", "--", path])?;
  }
  Ok(())
}

/// Commits the merge in progress in `worktree` once every conflicted file is resolved, the run's branch still at
/// `expected` (its last checkpoint) and the merge one of `base`: the conflicted files and the tracked files changed
/// beside them are committed as the merge.
pub fn finish(
  runner: &impl CommandRunner,
  git: &Path,
  worktree: &Path,
  (base, expected): (&BranchName, &Revision),
) -> Result<MergedIn, ResolveError> {
  let resolution = resolution(runner, git, worktree)?.ok_or(ResolveError::NotInMerge)?;
  let markers = resolution.with_markers();
  if !markers.is_empty() {
    return Err(ResolveError::MarkersLeft(markers));
  }
  let undecided = resolution.undecided();
  if !undecided.is_empty() {
    return Err(ResolveError::Undecided(undecided));
  }
  let git = Git::new(runner, git, worktree);
  if git.stdout(&["rev-parse", "--verify", "HEAD"])? != expected.as_str() {
    return Err(ResolveError::BranchMoved);
  }
  let base = format!("refs/heads/{}", base.as_str());
  if !git.succeeds(&["merge-base", "--is-ancestor", "MERGE_HEAD", &base])? {
    return Err(ResolveError::NotTheBase);
  }
  let unmerged: Vec<String> = staged(&git, &["ls-files", "--unmerged", "-z"])?.into_keys().collect();
  if !unmerged.is_empty() {
    let mut add = vec!["add", "--"];
    add.extend(unmerged.iter().map(String::as_str));
    git.stdout(&add)?;
  }
  git.stdout(&["add", "-u"])?;
  git.stdout(&["commit", "--no-edit"])?;
  Ok(merged_in(&git)?)
}

/// Ends the merge in progress in `worktree` and puts the worktree back as it was before it.
pub fn abort(runner: &impl CommandRunner, git: &Path, worktree: &Path) -> Result<(), ResolveError> {
  let git = Git::new(runner, git, worktree);
  if !merging(&git)? {
    return Err(ResolveError::NotInMerge);
  }
  git.stdout(&["merge", "--abort"])?;
  Ok(())
}

/// Whether `text` still holds a conflict marker at the start of a line.
pub fn holds_markers(text: &str) -> bool {
  text
    .lines()
    .any(|line| MARKERS.iter().any(|marker| line.starts_with(marker)))
}

/// How the unmerged `path` with index `stages` stands, read from the worktree.
fn unmerged_state(worktree: &Path, path: &str, stages: &[u8]) -> Result<FileState, ResolveError> {
  if !stages.contains(&RUN_STAGE) {
    return Ok(FileState::Undecided(Undecided::DeletedByRun));
  }
  if !stages.contains(&BASE_STAGE) {
    return Ok(FileState::Undecided(Undecided::DeletedByBase));
  }
  let unreadable = |error: std::io::Error| ResolveError::Unreadable {
    path: path.to_owned(),
    detail: error.to_string(),
  };
  let mut probe = Vec::new();
  match fs::File::open(worktree.join(path)) {
    Ok(file) => file.take(BINARY_PROBE).read_to_end(&mut probe).map_err(unreadable)?,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
      return Ok(FileState::Undecided(Undecided::Unmergeable));
    }
    Err(error) => return Err(unreadable(error)),
  };
  if probe.contains(&0) {
    return Ok(FileState::Undecided(Undecided::Unmergeable));
  }
  let text = fs::read(worktree.join(path)).map_err(unreadable)?;
  Ok(if holds_markers(&String::from_utf8_lossy(&text)) {
    FileState::Markers
  } else {
    FileState::Resolved
  })
}

/// The merge commit at the worktree's `HEAD` and the base commit it merged in, its second parent.
fn merged_in<R: CommandRunner>(git: &Git<'_, R>) -> Result<MergedIn, GitError> {
  let revision = |name: &str| {
    let commit = git.stdout(&["rev-parse", "--verify", "--end-of-options", name])?;
    Revision::new(&commit).map_err(|_| GitError::Unrecognized {
      expected: "a commit",
      detail: commit,
    })
  };
  Ok(MergedIn {
    base: revision("HEAD^2")?,
    tip: revision("HEAD")?,
  })
}

fn merging<R: CommandRunner>(git: &Git<'_, R>) -> Result<bool, GitError> {
  git.succeeds(&["rev-parse", "--quiet", "--verify", "MERGE_HEAD"])
}

/// The paths of an `ls-files -z` listing with stage information (`<mode> <object> <stage>\t<path>`), with their
/// stages, in path order.
fn staged<R: CommandRunner>(git: &Git<'_, R>, args: &[&str]) -> Result<BTreeMap<String, Vec<u8>>, GitError> {
  let output = git.output(args)?;
  if !output.success() {
    return Err(GitError::Failed(message(&output)));
  }
  let mut paths: BTreeMap<String, Vec<u8>> = BTreeMap::new();
  for entry in output.stdout.split('\0').filter(|entry| !entry.is_empty()) {
    let Some((info, path)) = entry.split_once('\t') else {
      continue;
    };
    let stage = info
      .rsplit(' ')
      .next()
      .and_then(|stage| stage.parse::<u8>().ok())
      .ok_or_else(|| GitError::Unrecognized {
        expected: "an index stage",
        detail: info.to_owned(),
      })?;
    paths.entry(path.to_owned()).or_default().push(stage);
  }
  Ok(paths)
}
