//! A repository's changes read with git, read-only: the diff between two commits, stored once per
//! pair since it does not change, and what a worktree holds beyond its checked-out commit,
//! untracked files included. A change larger than [`Changes::is_too_large`] is measured
//! (`--numstat`) but not read, as for pull requests. Git runs with options that keep its output
//! the unified diff [`super::Diff::parse`] reads whatever the user's configuration says: no
//! colors, external diff drivers or text conversion, and the `a/` and `b/` prefixes.

use std::path::Path;

use thiserror::Error;

use super::{Changes, Diff, RevisionDiff};
use crate::{
  command::{Command, CommandError, CommandRunner},
  repository::redact_userinfo,
  revision::Revision,
  store::RevisionDiffStore,
  time::Timestamp,
};

const DIFF_OPTIONS: [&str; 6] = [
  "--no-color",
  "--no-ext-diff",
  "--no-textconv",
  "--find-renames",
  "--src-prefix=a/",
  "--dst-prefix=b/",
];
/// What `git diff --no-index` compares an untracked file with, on every platform.
const NULL_FILE: &str = "/dev/null";

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum GitDiffError {
  #[error("cannot run git: {0}")]
  Run(#[from] CommandError),
  /// Git refused, e.g. a revision that no longer exists; its message, without credentials.
  #[error("git failed: {0}")]
  Failed(String),
  #[error("git printed something that is not {expected}: {detail}")]
  Unrecognized { expected: &'static str, detail: String },
}

#[derive(Debug, Error)]
pub enum RevisionDiffError<E> {
  #[error(transparent)]
  Git(#[from] GitDiffError),
  #[error("cannot read or store the diff: {0}")]
  Store(E),
}

/// What reading the diff between two commits found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RevisionChanges {
  Read(RevisionDiff),
  /// Larger than Orchester reads; only its size was.
  TooLarge(Changes),
}

/// What a worktree holds beyond `head`, the commit it has checked out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorktreeChanges {
  pub head: Revision,
  pub read: WorktreeRead,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorktreeRead {
  Clean,
  /// The unified diff from `head` to the files in the worktree; untracked files are added files.
  Read(String),
  /// Larger than Orchester reads. The lines of untracked files are not counted when there are too
  /// many files to read them.
  TooLarge(Changes),
}

/// The diff from `base` to `head` in the repository at `repo`: the one `store` holds, else read
/// with the `git` program and stored, read at `now`.
pub fn read_revision_diff<S: RevisionDiffStore>(
  runner: &impl CommandRunner,
  git: &Path,
  repo: &Path,
  store: &mut S,
  (base, head): (&Revision, &Revision),
  now: Timestamp,
) -> Result<RevisionChanges, RevisionDiffError<S::Error>> {
  if let Some(stored) = store.revision_diff(base, head).map_err(RevisionDiffError::Store)? {
    return Ok(RevisionChanges::Read(stored));
  }
  let range = [base.as_str(), head.as_str()];
  let size = parse_numstat(&run(runner, &git_numstat_command(git, repo, &range))?)?;
  if size.is_too_large() {
    return Ok(RevisionChanges::TooLarge(size));
  }
  let diff = RevisionDiff {
    base: base.clone(),
    head: head.clone(),
    text: run(runner, &git_diff_command(git, repo, &range))?,
    read_at: now,
  };
  store.save_revision_diff(&diff).map_err(RevisionDiffError::Store)?;
  Ok(RevisionChanges::Read(diff))
}

/// What the worktree at `worktree` changed beyond its checked-out commit, read with `git`.
pub fn read_worktree_changes(
  runner: &impl CommandRunner,
  git: &Path,
  worktree: &Path,
) -> Result<WorktreeChanges, GitDiffError> {
  let head = run(runner, &git_command(git, worktree, ["rev-parse", "--verify", "HEAD"]))?;
  let head = Revision::new(head.trim()).map_err(|error| GitDiffError::Unrecognized {
    expected: "a commit",
    detail: error.to_string(),
  })?;
  let tracked = parse_numstat(&run(runner, &git_numstat_command(git, worktree, &["HEAD"]))?)?;
  let listed = run(
    runner,
    &git_command(git, worktree, ["ls-files", "--others", "--exclude-standard", "-z"]),
  )?;
  let untracked: Vec<&str> = listed.split('\0').filter(|path| !path.is_empty()).collect();
  if tracked.files == 0 && untracked.is_empty() {
    return Ok(WorktreeChanges {
      head,
      read: WorktreeRead::Clean,
    });
  }
  // More untracked files than a u32 counts are as many as it counts: far too many either way.
  let untracked_files = u32::try_from(untracked.len()).unwrap_or(u32::MAX);
  let mut size = Changes {
    files: tracked.files.saturating_add(untracked_files),
    ..tracked
  };
  if size.is_too_large() {
    return Ok(WorktreeChanges {
      head,
      read: WorktreeRead::TooLarge(size),
    });
  }
  let mut text = run(runner, &git_diff_command(git, worktree, &["HEAD"]))?;
  for path in untracked {
    let (diff, added) = untracked_diff(runner, git, worktree, path)?;
    size.additions = size.additions.saturating_add(added);
    text.push_str(&diff);
  }
  let read = if size.is_too_large() {
    WorktreeRead::TooLarge(size)
  } else {
    WorktreeRead::Read(text)
  };
  Ok(WorktreeChanges { head, read })
}

/// `git diff` of the untracked file `path` as an added file, and its lines. Comparing two files,
/// git exits with 1 when they differ, which a new file always does: its output decides.
fn untracked_diff(
  runner: &impl CommandRunner,
  git: &Path,
  worktree: &Path,
  path: &str,
) -> Result<(String, u32), GitDiffError> {
  let mut args = vec!["diff", "--no-index"];
  args.extend(DIFF_OPTIONS);
  args.extend(["--", NULL_FILE, path]);
  let output = runner.run(&git_command(git, worktree, args))?;
  if !output.success() && !output.stdout.starts_with("diff --git ") {
    return Err(GitDiffError::Failed(redact_userinfo(output.stderr.trim())));
  }
  let added = Diff::parse(&output.stdout)
    .map_err(|error| GitDiffError::Unrecognized {
      expected: "a unified diff",
      detail: error.to_string(),
    })?
    .additions();
  Ok((output.stdout, added))
}

/// `git <args>` in `directory`, with git's messages in English. Git takes no optional locks: a read
/// beside an agent at work must not refresh the worktree's index while the runner commits in it.
pub fn git_command<'a>(git: &Path, directory: &Path, args: impl IntoIterator<Item = &'a str>) -> Command {
  Command::new(git, args)
    .with_current_dir(directory)
    .with_env("LC_ALL", "C")
    .with_env("GIT_OPTIONAL_LOCKS", "0")
}

/// Variables that would point git at another repository, index, object store or ref namespace than
/// the folder it runs in, or give it configuration of the caller's, e.g. ones a hook that started
/// Orchester's process passed on. Without `GIT_CONFIG_COUNT`, git reads no `GIT_CONFIG_KEY_<n>` or
/// `GIT_CONFIG_VALUE_<n>`.
pub const LOCATING: [&str; 9] = [
  "GIT_DIR",
  "GIT_WORK_TREE",
  "GIT_INDEX_FILE",
  "GIT_OBJECT_DIRECTORY",
  "GIT_ALTERNATE_OBJECT_DIRECTORIES",
  "GIT_COMMON_DIR",
  "GIT_NAMESPACE",
  "GIT_CONFIG_PARAMETERS",
  "GIT_CONFIG_COUNT",
];

/// [`git_command`] without the caller's [`LOCATING`] variables: git works on what it finds from
/// `directory` only.
pub fn local_git_command<'a>(git: &Path, directory: &Path, args: impl IntoIterator<Item = &'a str>) -> Command {
  LOCATING
    .iter()
    .fold(git_command(git, directory, args), |command, name| {
      command.without_env(name)
    })
}

/// `git diff --numstat` of `revisions` (a range's two ends, or the one a worktree is compared
/// with) in `directory`.
pub fn git_numstat_command(git: &Path, directory: &Path, revisions: &[&str]) -> Command {
  let mut args = vec!["diff", "--numstat", "-z", "--find-renames"];
  args.extend(revisions);
  args.push("--");
  git_command(git, directory, args)
}

/// `git diff` of `revisions` in `directory`, as [`git_numstat_command`] measures it.
pub fn git_diff_command(git: &Path, directory: &Path, revisions: &[&str]) -> Command {
  let mut args = vec!["diff"];
  args.extend(DIFF_OPTIONS);
  args.extend(revisions);
  args.push("--");
  git_command(git, directory, args)
}

/// The size of the change `git diff --numstat -z` lists: `<added>\t<removed>\t<path>` per file,
/// a renamed file with an empty path followed by its old and new path, `-` for the lines of a
/// binary file.
pub fn parse_numstat(stdout: &str) -> Result<Changes, GitDiffError> {
  let unrecognized = |entry: &str| GitDiffError::Unrecognized {
    expected: "a numstat entry",
    detail: entry.to_owned(),
  };
  let count = |value: &str, entry: &str| match value {
    "-" => Ok(0),
    value => value.parse::<u32>().map_err(|_| unrecognized(entry)),
  };
  let mut size = Changes::default();
  let mut fields = stdout.split('\0');
  while let Some(entry) = fields.next() {
    if entry.is_empty() {
      continue;
    }
    let mut parts = entry.splitn(3, '\t');
    let (Some(added), Some(removed), Some(path)) = (parts.next(), parts.next(), parts.next()) else {
      return Err(unrecognized(entry));
    };
    if path.is_empty() && (fields.next().is_none() || fields.next().is_none()) {
      return Err(unrecognized(entry));
    }
    size.files = size.files.saturating_add(1);
    size.additions = size.additions.saturating_add(count(added, entry)?);
    size.deletions = size.deletions.saturating_add(count(removed, entry)?);
  }
  Ok(size)
}

/// Standard output of a git command that must succeed.
fn run(runner: &impl CommandRunner, command: &Command) -> Result<String, GitDiffError> {
  let output = runner.run(command)?;
  if output.success() {
    Ok(output.stdout)
  } else {
    Err(GitDiffError::Failed(redact_userinfo(output.stderr.trim())))
  }
}
