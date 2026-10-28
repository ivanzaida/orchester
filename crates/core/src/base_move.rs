//! When the base branch moves (ORC-54): runs work on branches made from their base branch when they started, and
//! meanwhile other runs are merged into it, by Orchester or outside it. Each time the main checkout's base branch is
//! seen at a new head ([`BaseHead::observe`]), every unfinished run on that base whose branch lacks the new head gets
//! a notice ([`BaseNotice`]) that its writing agent is to merge the base into its worktree, while the conflicts are
//! small. Moves within [`COALESCE_MILLIS`] of the first one are one move: the run's notice waiting to be delivered is
//! replaced by the newer one and is delivered only once the window has passed, so a burst of merges is one notice.

use serde::{Deserialize, Serialize};

use crate::{
  id::{InvocationId, NoticeId, RunId, StepExecutionId, TaskId},
  revision::Revision,
  run::BranchName,
  time::Timestamp,
};

/// How long after a base branch's first move further moves count as the same one: two minutes.
pub const COALESCE_MILLIS: i64 = 2 * 60 * 1000;

/// Where a base branch's head was last seen, and when the move that put it there began; `None` before a move was
/// seen: the branch's first sighting starts no window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BaseHead {
  pub head: Revision,
  pub moved_at: Option<Timestamp>,
}

/// What seeing a base branch at a head tells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Observed {
  /// It is where it was seen last.
  Unchanged,
  /// It was never seen before: there is no move to tell, only where it is from now on.
  First(BaseHead),
  /// It moved, in a move that began at `moved_at`: within the window of the move before, it is that move still.
  Moved { head: BaseHead, moved_at: Timestamp },
}

impl BaseHead {
  /// What seeing the branch at `head` at `now` tells, after it was seen as `previous`.
  pub fn observe(previous: Option<&Self>, head: Revision, now: Timestamp) -> Observed {
    let Some(seen) = previous else {
      return Observed::First(Self { head, moved_at: None });
    };
    if seen.head == head {
      return Observed::Unchanged;
    }
    let moved_at = seen
      .moved_at
      .filter(|began| now.unix_millis().saturating_sub(began.unix_millis()) < COALESCE_MILLIS)
      .unwrap_or(now);
    Observed::Moved {
      head: Self {
        head,
        moved_at: Some(moved_at),
      },
      moved_at,
    }
  }
}

/// When the notices of a move that began at `moved_at` may reach the agents: once its window has passed.
pub const fn deliver_after(moved_at: Timestamp) -> Timestamp {
  Timestamp::from_unix_millis(moved_at.unix_millis().saturating_add(COALESCE_MILLIS))
}

/// A commit that landed on the base branch and that a run's branch does not have.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LandedCommit {
  pub revision: Revision,
  /// Its subject line, as written: data, never instructions.
  pub subject: String,
}

/// A task of the project whose work landed, as a commit subject names it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LandedTask {
  pub key: String,
  pub title: String,
}

/// Where a notice stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoticeDelivery {
  /// It waits for an agent of a writing step of its run, for the runner to merge it, or for the window of its move to
  /// pass.
  Pending,
  /// Invocation `invocation` of `execution` started with it at `at`.
  Delivered {
    execution: StepExecutionId,
    invocation: InvocationId,
    at: Timestamp,
  },
  /// The runner merged it into the run's worktree itself at `at`, with the merge commit `revision`, as `execution`
  /// started (ORC-151): it overlapped none of the run's changes, so no agent was interrupted for it.
  Merged {
    execution: StepExecutionId,
    revision: Revision,
    at: Timestamp,
  },
}

/// A notice to run `run` that its base branch `branch` moved to `revision`, which its branch lacks: what landed,
/// the files that also changed on the run's side (likely conflicts), and where it stands. A run has at most one
/// pending notice; a newer move replaces it, so pending moves combine into one.
///
/// Only a notice that overlaps the run's changes interrupts a writing step's agent (ORC-151); the runner merges one
/// that does not itself, before the run's next step that does not take base moves, and delivers it to a writing step
/// as one that overlaps when that merge conflicts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BaseNotice {
  pub id: NoticeId,
  pub run: RunId,
  pub task: TaskId,
  pub branch: BranchName,
  pub revision: Revision,
  /// The commits of the base the run's branch lacks, newest first, at most [`crate::integration::MAX_LANDED`].
  pub commits: Vec<LandedCommit>,
  pub tasks: Vec<LandedTask>,
  /// Files changed on both sides since they forked, in path order; after the runner's merge conflicted, the files it
  /// conflicted in.
  pub overlapping: Vec<String>,
  pub noticed_at: Timestamp,
  pub deliver_after: Timestamp,
  pub delivery: NoticeDelivery,
}

impl BaseNotice {
  pub fn is_pending(&self) -> bool {
    self.delivery == NoticeDelivery::Pending
  }

  /// Whether an agent starting at `now` takes it: it is pending and the window of its move has passed.
  pub fn deliverable(&self, now: Timestamp) -> bool {
    self.is_pending() && now >= self.deliver_after
  }

  /// When it was delivered to an agent, once it was.
  pub const fn delivered(&self) -> Option<(StepExecutionId, Timestamp)> {
    match self.delivery {
      NoticeDelivery::Pending | NoticeDelivery::Merged { .. } => None,
      NoticeDelivery::Delivered { execution, at, .. } => Some((execution, at)),
    }
  }

  /// The merge commit, and when the runner merged it itself, once it did.
  pub const fn merged(&self) -> Option<(&Revision, Timestamp)> {
    match &self.delivery {
      NoticeDelivery::Pending | NoticeDelivery::Delivered { .. } => None,
      NoticeDelivery::Merged { revision, at, .. } => Some((revision, *at)),
    }
  }

  /// The execution it was delivered to or merged before, and when, once it is no longer pending.
  pub const fn settled(&self) -> Option<(StepExecutionId, Timestamp)> {
    match self.delivery {
      NoticeDelivery::Pending => None,
      NoticeDelivery::Delivered { execution, at, .. } | NoticeDelivery::Merged { execution, at, .. } => {
        Some((execution, at))
      }
    }
  }
}

/// Those of `tasks` (key, title) that `commits`' subjects name by their key as a word of its own, each once, in
/// `tasks`' order.
pub fn landed_tasks(commits: &[LandedCommit], tasks: impl IntoIterator<Item = (String, String)>) -> Vec<LandedTask> {
  tasks
    .into_iter()
    .filter(|(key, _)| commits.iter().any(|commit| names(&commit.subject, key)))
    .map(|(key, title)| LandedTask { key, title })
    .collect()
}

/// Whether `subject` names `key` as a word of its own: `ORC-2` in "Merge ORC-2 run #1" or "ORC-2: Implement", not
/// in "ORC-21".
fn names(subject: &str, key: &str) -> bool {
  subject.match_indices(key).any(|(at, _)| {
    let before = subject[..at].chars().next_back();
    let after = subject[at + key.len()..].chars().next();
    before.is_none_or(|c| !c.is_alphanumeric() && c != '-') && after.is_none_or(|c| !c.is_alphanumeric())
  })
}
