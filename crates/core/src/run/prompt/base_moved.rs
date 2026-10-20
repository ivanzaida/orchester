//! What a writing agent is told when its run's base branch moved (ORC-54, `crate::base_move`): how to merge the base
//! safely, in every writing step's *Files and commands* part ([`MERGE_SAFELY`]), and the notice itself
//! ([`notice_text`]) when an invocation delivers one: in a new session's task part, and first in a resumed one, after
//! the turn it finished or with the continue prompt when it was stopped. Commit subjects and file names are quoted as
//! data.

use std::fmt::Write as _;

use super::{CONTINUE_PROMPT, composition::push_block};
use crate::base_move::BaseNotice;

/// How a writing agent merges its base branch, should Orchester tell it that the base moved.
pub const MERGE_SAFELY: &str = "When Orchester tells you that the base branch moved, merge it into this worktree \
                                with git: `git merge <base>` here, never `git stash`, and never rebase or force-push \
                                a branch others share. Resolve conflicts keeping both sides' intent, commit the \
                                merge as a checkpoint, then run the filtered tests the merged changes affect.";

/// A notice an invocation delivers; `after_turn`: its agent finished its turn and its session resumes to read it.
#[derive(Clone, Copy, Debug)]
pub struct BaseMoved<'a> {
  pub notice: &'a BaseNotice,
  pub after_turn: bool,
}

/// What the agent is told of `notice`: where the base moved, what landed, the files both sides changed, and to merge.
pub fn notice_text(notice: &BaseNotice) -> String {
  let branch = notice.branch.as_str();
  let mut text = format!(
    "Orchester: the base branch `{branch}` moved to {} while you worked, and this worktree does not have it yet.\n",
    notice.revision
  );
  if !notice.tasks.is_empty() {
    text.push_str("\nTasks that landed:\n");
    for task in &notice.tasks {
      // Writing to a String cannot fail.
      let _ = writeln!(text, "- {}: {}", task.key, task.title);
    }
  }
  if !notice.commits.is_empty() {
    text.push_str("\nCommits that landed, newest first:\n");
    for commit in &notice.commits {
      let short = commit.revision.as_str().get(..10).unwrap_or_default();
      let _ = writeln!(text, "- {short} {}", commit.subject);
    }
  }
  if notice.overlapping.is_empty() {
    text.push_str("\nNo file they changed is one this run changed too.\n");
  } else {
    text.push_str("\nFiles they changed that this run changed too, where conflicts are likely:\n");
    for path in &notice.overlapping {
      let _ = writeln!(text, "- `{path}`");
    }
  }
  let _ = write!(
    text,
    "\nMerge `{branch}` into this worktree now with git (`git merge {branch}`; never `git stash`, never rebase a \
     shared branch). Resolve the conflicts keeping both sides' intent, commit the merge, and run the filtered tests \
     the merged changes affect. Then go on with your step, and say in your result what you merged and how you \
     resolved the conflicts."
  );
  text
}

/// The notice under its heading, in a new session's task part.
pub(super) fn push_task(text: &mut String, moved: Option<BaseMoved<'_>>) {
  let Some(moved) = moved else {
    return;
  };
  text.push_str("\n# The base branch moved\n");
  push_block(text, &notice_text(moved.notice));
}

/// What a resumed session reads first when it is told only of the notice: the notice, after the continue prompt
/// when its agent was stopped rather than finished its turn.
pub(super) fn push_resumed(text: &mut String, moved: BaseMoved<'_>) {
  if !moved.after_turn {
    text.push_str(CONTINUE_PROMPT);
    text.push_str("\n\n");
  }
  text.push_str(&notice_text(moved.notice));
}
