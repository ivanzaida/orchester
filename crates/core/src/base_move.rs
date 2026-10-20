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

/// Where a base branch's head was last seen, and when the move that put it there began.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BaseHead {
  pub head: Revision,
  pub moved_at: Timestamp,
}

/// What seeing a base branch at a head tells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Observed {
  /// It is where it was seen last.
  Unchanged,
  /// It was never seen before: there is no move to tell, only where it is from now on.
  First(BaseHead),
  /// It moved; within the window of the move before, it is that move still.
  Moved(BaseHead),
}

impl BaseHead {
  /// What seeing the branch at `head` at `now` tells, after it was seen as `previous`.
  pub fn observe(previous: Option<&Self>, head: Revision, now: Timestamp) -> Observed {
    match previous {
      None => Observed::First(Self { head, moved_at: now }),
      Some(seen) if seen.head == head => Observed::Unchanged,
      Some(seen) if now.unix_millis().saturating_sub(seen.moved_at.unix_millis()) < COALESCE_MILLIS => {
        Observed::Moved(Self {
          head,
          moved_at: seen.moved_at,
        })
      }
      Some(_) => Observed::Moved(Self { head, moved_at: now }),
    }
  }

  /// When the notices of this move may reach the agents: once the window of the move has passed.
  pub const fn deliver_after(&self) -> Timestamp {
    Timestamp::from_unix_millis(self.moved_at.unix_millis().saturating_add(COALESCE_MILLIS))
  }
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeDelivery {
  /// It waits for an agent of a writing step of its run, or for the window of its move to pass.
  Pending,
  /// Invocation `invocation` of `execution` started with it at `at`.
  Delivered {
    execution: StepExecutionId,
    invocation: InvocationId,
    at: Timestamp,
  },
}

/// A notice to run `run` that its base branch `branch` moved to `revision`, which its branch lacks: what landed,
/// the files that also changed on the run's side (likely conflicts), and where it stands. A run has at most one
/// pending notice; a newer move replaces it.
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
  /// Files changed on both sides since they forked, in path order.
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

  /// When it was delivered, once it was.
  pub const fn delivered(&self) -> Option<(StepExecutionId, Timestamp)> {
    match self.delivery {
      NoticeDelivery::Pending => None,
      NoticeDelivery::Delivered { execution, at, .. } => Some((execution, at)),
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
