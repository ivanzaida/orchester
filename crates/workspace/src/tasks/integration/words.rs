//! What a person reads of a merge or a removal that did not happen: the words of each cause, the files and paths it
//! names shortened as the app names them.

use orchester_core::{
  command::CommandError,
  integration::{FolderListing, GitError, LeftWorktree, MergeError, MergeStep},
};

use crate::messages::Message;

/// Names shown of a longer list of files, which then ends in an ellipsis.
const SHOWN_FILES: usize = 5;
/// Characters of git's reason shown under a row or in a dialog; the log keeps all of it.
const REASON_CHARS: usize = 160;

pub(super) fn merge_message(error: &MergeError, separator: &str) -> Message {
  let with_files = |key, files: &[String]| Message::new(key).with("files", file_list(files, separator));
  match error {
    MergeError::NotSucceeded => Message::new("runs.merge.failure.notSucceeded"),
    MergeError::NoBaseBranch => Message::new("runs.merge.failure.noBaseBranch"),
    MergeError::WrongBranch {
      expected,
      current: Some(current),
    } => Message::new("runs.merge.failure.wrongBranch")
      .with("branch", expected.to_string())
      .with("current", current.to_string()),
    MergeError::WrongBranch {
      expected,
      current: None,
    } => Message::new("runs.merge.failure.detached").with("branch", expected.to_string()),
    MergeError::UncommittedInTheWay { files } => with_files("runs.merge.failure.uncommitted", files),
    MergeError::InProgress => Message::new("runs.merge.failure.inProgress"),
    MergeError::BaseMoved { branch } => Message::new("runs.merge.failure.baseMoved").with("branch", branch.to_string()),
    MergeError::LeftChanged { files, detail } => {
      with_files("runs.merge.failure.leftChanged", files).with("detail", detail.clone())
    }
    MergeError::NotPutBack { files, .. } => with_files("runs.merge.failure.notPutBack", files),
    MergeError::Unanswered { step, command, after } => Message::new(unanswered_key(*step))
      .with("command", command.clone())
      .with("seconds", after.as_secs().to_string()),
    MergeError::BranchMissing(branch) => {
      Message::new("runs.merge.failure.branchMissing").with("branch", branch.to_string())
    }
    MergeError::BaseMissing(branch) => {
      Message::new("runs.merge.failure.baseMissing").with("branch", branch.to_string())
    }
    MergeError::CheckedOutIn { branch, path } => Message::new("runs.merge.failure.checkedOutIn")
      .with("branch", branch.to_string())
      .with("path", path.clone()),
    MergeError::BranchMoved { branch, expected } => Message::new("runs.merge.failure.branchMoved")
      .with("branch", branch.to_string())
      .with("revision", crate::display::short_revision(expected.as_str()).to_owned()),
    MergeError::Conflict { files } => with_files("runs.merge.failure.conflict", files),
    MergeError::Refused(said) => Message::new("runs.merge.failure.refused").with("detail", said.clone()),
    MergeError::Git(error) => git_message(error),
  }
}

/// What a git that stopped answering at `step` left as it was, and what to do.
const fn unanswered_key(step: MergeStep) -> &'static str {
  match step {
    MergeStep::Check => "runs.merge.failure.unanswered.check",
    MergeStep::TemporaryWorktree => "runs.merge.failure.unanswered.temporaryWorktree",
    MergeStep::Merge => "runs.merge.failure.unanswered.merge",
    MergeStep::BringIn => "runs.merge.failure.unanswered.bringIn",
    MergeStep::PutBack => "runs.merge.failure.unanswered.putBack",
  }
}

pub(crate) fn git_message(error: &GitError) -> Message {
  match error {
    GitError::Run(CommandError::NotFound) => Message::new("runs.changes.failure.gitMissing"),
    GitError::Run(_) | GitError::Unrecognized { .. } => Message::new("runs.merge.failure.gitStopped"),
    GitError::Unanswered { command, after } => Message::new("runs.merge.failure.gitUnanswered")
      .with("command", command.clone())
      .with("seconds", after.as_secs().to_string()),
    GitError::Failed(said) => Message::new("runs.merge.failure.git").with("detail", said.clone()),
  }
}

/// Why the listing of a worktree folder git forgot stopped at an entry that could not be read, for
/// its run's row and the removal's dialog; `None` when it was read.
pub fn unread_entry(worktree: &LeftWorktree) -> Option<Message> {
  let FolderListing::Unreadable { path, detail } = &worktree.listing else {
    return None;
  };
  Some(
    Message::new("runs.worktree.failure.folderUnreadable")
      .with("path", short_path(path))
      .with("detail", short_reason(detail, path)),
  )
}

/// A worktree's path as the app names it elsewhere, from `.orchester/worktrees/` on.
pub(super) fn short_path(path: &str) -> String {
  let path = path.replace('\\', "/");
  path
    .find(".orchester/")
    .map_or_else(|| path.clone(), |start| path[start..].to_owned())
}

/// Git's reason for not removing the worktree at `path`, on one line: its first line without
/// git's `error:` or `fatal:`, the path shortened as [`short_path`] does, at most [`REASON_CHARS`].
pub(super) fn short_reason(detail: &str, path: &str) -> String {
  let line = detail
    .replace('\\', "/")
    .replace(&path.replace('\\', "/"), &short_path(path))
    .lines()
    .map(str::trim)
    .find(|line| !line.is_empty())
    .unwrap_or_default()
    .to_owned();
  let line = ["error: ", "fatal: "]
    .iter()
    .find_map(|prefix| line.strip_prefix(prefix))
    .unwrap_or(&line)
    .to_owned();
  // "failed to delete '<path>': Invalid argument": the message names the path already.
  let line = match line.rsplit_once(": ") {
    Some((before, reason)) if before.contains(&short_path(path)) => reason.to_owned(),
    _ => line,
  };
  if line.chars().count() <= REASON_CHARS {
    return line;
  }
  let mut short: String = line.chars().take(REASON_CHARS).collect();
  short.push('…');
  short
}

/// At most [`SHOWN_FILES`] of `files`, joined with `separator`.
pub fn file_list(files: &[String], separator: &str) -> String {
  let mut list = files
    .iter()
    .take(SHOWN_FILES)
    .cloned()
    .collect::<Vec<_>>()
    .join(separator);
  if files.len() > SHOWN_FILES {
    list.push('…');
  }
  list
}
