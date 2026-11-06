//! What a writing agent is told when its run's base branch moved (ORC-54, `crate::base_move`): what Orchester does
//! with the base when it tells it so, in every writing step's *Files and commands* part ([`MERGE_SAFELY`]), and the
//! notice itself ([`notice_text`]) when an invocation delivers one: in a new session's task part, and first in a
//! resumed one, after the turn it finished or with the continue prompt when it was stopped. Commit subjects and file
//! names are quoted as data.
//!
//! The runner merges the moved base into the agent's worktree before the agent reads the notice ([`BaseMerge`]): a
//! sandboxed agent may write its worktree's files but not the repository's git data (its index lock, objects and
//! branch refs live in the main checkout's `.git`), so it could not merge or commit; it only resolves the conflicts
//! git leaves in the files, and the runner commits the merge with the step's work.

use std::fmt::Write as _;

use super::{CONTINUE_PROMPT, composition::push_block};
use crate::{base_move::BaseNotice, revision::Revision};

/// What a writing agent does when Orchester tells it that the base moved: Orchester has merged it already, so the
/// agent runs no git command that merges, commits or moves the branch, and resolves conflicts by editing files.
pub const MERGE_SAFELY: &str = "When Orchester tells you that the base branch moved, it has already merged the base \
                                into this worktree: never run `git merge`, `git rebase`, `git stash`, `git reset`, \
                                `git add` or `git commit` for it. Where git stopped at conflicts, resolve them by \
                                editing the files it names, keeping both sides' intent, until no conflict marker is \
                                left; Orchester commits the merge with your work. Then run the filtered tests the \
                                merged changes affect.";

/// How the runner brought the moved base into the agent's worktree before the agent reads the notice.
#[derive(Clone, Copy, Debug)]
pub enum BaseMerge<'a> {
  /// Git merged it without conflicts, with this merge commit, checked out now.
  Merged(&'a Revision),
  /// Git stopped at conflicts in these files, which hold its conflict markers; the merge stays in progress until the
  /// runner commits it with the step's work.
  Conflicted(&'a [String]),
}

/// A notice an invocation delivers and how its base was merged; `after_turn`: its agent finished its turn and its
/// session resumes to read it.
#[derive(Clone, Copy, Debug)]
pub struct BaseMoved<'a> {
  pub notice: &'a BaseNotice,
  pub merge: BaseMerge<'a>,
  pub after_turn: bool,
}

/// What the agent is told of `notice`, merged as `merge` says: where the base moved, what landed, the files both sides
/// changed, and what is left for it to do.
pub fn notice_text(notice: &BaseNotice, merge: BaseMerge<'_>) -> String {
  let branch = notice.branch.as_str();
  let mut text = format!(
    "Orchester: the base branch `{branch}` moved to {} while you worked, and Orchester merged it into this worktree.\n",
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
    text.push_str("\nFiles they changed that this run changed too:\n");
    for path in &notice.overlapping {
      let _ = writeln!(text, "- `{path}`");
    }
  }
  push_merge(&mut text, branch, merge);
  text
}

/// What is left of the merge of `branch` for the agent: checking a clean one, or resolving the conflicts of one in
/// progress. Also what a step a merge's conflicts were returned to reads of the merge the runner began for it.
pub(super) fn push_merge(text: &mut String, branch: &str, merge: BaseMerge<'_>) {
  match merge {
    BaseMerge::Merged(revision) => {
      let short = revision.as_str().get(..10).unwrap_or_default();
      let _ = write!(
        text,
        "\nGit merged `{branch}` without conflicts, as merge commit {short}. Check that your work still fits what \
         landed, above all in the files both sides changed, and run the filtered tests the merged changes affect. \
         Then go on with your step, and say in your result what the merge changed for your work."
      );
    }
    BaseMerge::Conflicted(files) => {
      text.push_str(
        "\nGit stopped at conflicts in these files, which now hold its conflict markers (`<<<<<<<`, `=======`, \
         `>>>>>>>`):\n",
      );
      for path in files {
        let _ = writeln!(text, "- `{path}`");
      }
      text.push_str(
        "\nResolve each conflict by editing the file, keeping both sides' intent, until no conflict marker is left, \
         and run the filtered tests the merged changes affect. Run no `git merge`, `git add`, `git commit`, `git \
         stash`, `git reset` or `git checkout`: the merge stays in progress, and Orchester commits it with your work \
         when your step ends; a conflict marker left then fails the step. Then go on with your step, and say in \
         your result how you resolved the conflicts.",
      );
    }
  }
}

/// The notice under its heading, in a new session's task part.
pub(super) fn push_task(text: &mut String, moved: Option<BaseMoved<'_>>) {
  let Some(moved) = moved else {
    return;
  };
  text.push_str("\n# The base branch moved\n");
  push_block(text, &notice_text(moved.notice, moved.merge));
}

/// What a resumed session reads first when it is told only of the notice: the notice, after the continue prompt
/// when its agent was stopped rather than finished its turn.
pub(super) fn push_resumed(text: &mut String, moved: BaseMoved<'_>) {
  if !moved.after_turn {
    text.push_str(CONTINUE_PROMPT);
    text.push_str("\n\n");
  }
  text.push_str(&notice_text(moved.notice, moved.merge));
}
