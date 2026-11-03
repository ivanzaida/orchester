//! What the review page and `get_run` read of a run's review: its threads, each with its state, the files marked
//! *Viewed* at the run's head, and whether Implement works on comments sent to it. Reading settles the comments
//! Implement addressed once its work ended: each becomes *addressed* in the first commit after the thread's head that
//! changed its lines (none when no commit did), with the words Implement's step ended with. A thread whose lines a
//! later head changed is *outdated*, folded with the commit that changed them: the run's diff up to each commit after
//! the thread's head is read (each pair of commits once, then from storage), and the first in which the thread's code
//! is no longer among the lines it shows is that commit ([`locate`]). Blocks on SQLite and git.

use std::path::Path;

use orchester_core::{
  id::RunId,
  integration::{branch_head, own_commits},
  pull_request::feedback::{Located, locate},
  review::{CommentState, ReviewMessage, ReviewThread, Said, ThreadState, Viewed},
  revision::Revision,
  run::Run,
  store::{ActivityStore, RunStore},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;

use super::{ReviewRefusal, implement_at_work, recover};
use crate::tasks::{
  changes::{Checkpointed, GIT, RunRange, checkpointed_in, own_start},
  detail::run_store_error,
};

/// A thread as the page shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreadView {
  pub thread: ReviewThread,
  pub state: ThreadState,
}

/// A run's review as it stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Review {
  /// Where the run's branch is (its latest checkpoint once the branch is gone); `None` before its first.
  pub head: Option<Revision>,
  /// Oldest first.
  pub threads: Vec<ThreadView>,
  /// The files marked *Viewed* at `head`.
  pub viewed: Vec<Viewed>,
  /// Implement works on comments sent to it: *Approve and merge* is unavailable.
  pub implement_at_work: bool,
}

impl Review {
  /// The comments waiting in the tray: what *Send to Implement (N)* sends.
  pub fn unsent(&self) -> usize {
    self
      .threads
      .iter()
      .filter(|view| view.thread.resolved_at().is_none())
      .map(|view| view.thread.unsent().count())
      .sum()
  }
}

/// `run`'s review in the project at `root`, the comments Implement addressed settled first. `programs` read the run's
/// branch and diffs.
pub fn review_of(root: &Path, programs: &Programs, run: RunId) -> Result<Review, ReviewRefusal> {
  let mut db = ProjectDb::open(root)?;
  let stored = db.run(run).map_err(run_store_error)?.ok_or(ReviewRefusal::NotFound)?;
  let range = RunRange::of(&stored);
  let head = match branch_head(&programs.runner(), Path::new(GIT), (root, stored.branch())) {
    Ok(Some(tip)) => Some(tip),
    Ok(None) => range.head.clone(),
    Err(error) => {
      tracing::warn!(%error, %run, "cannot read where the run's branch is; its review reads its latest checkpoint");
      range.head.clone()
    }
  };
  let lines = Lines {
    root,
    programs,
    range: &range,
    head: head.as_ref(),
  };
  let mut threads = recover::threads(&mut db, run)?;
  if stored.status().is_terminal() && threads.iter().any(|thread| thread.sent().is_some()) {
    settle_addressed(&mut db, &lines, &stored, &threads)?;
    threads = db.review_threads(run)?;
  }
  let viewed = match &head {
    Some(head) => db.review_viewed(run, head)?,
    None => Vec::new(),
  };
  let implement_at_work = implement_at_work(&threads, &stored);
  let threads = threads
    .into_iter()
    .map(|thread| {
      let changed = (thread.resolved_at().is_none())
        .then(|| lines.changed_by(&thread))
        .flatten();
      let state = ThreadState::of(&thread, changed.as_ref());
      ThreadView { thread, state }
    })
    .collect();
  Ok(Review {
    head,
    threads,
    viewed,
    implement_at_work,
  })
}

/// Settles each comment of `threads` sent to Implement, whose work on `run` ended: addressed in the commit that changed
/// its lines, with the words of the first step that ended after it was sent.
fn settle_addressed(
  db: &mut ProjectDb,
  lines: &Lines<'_>,
  run: &Run,
  threads: &[ReviewThread],
) -> Result<(), ReviewRefusal> {
  let outputs = db.step_outputs(run.id())?;
  for thread in threads {
    let Some(comment) = thread.sent() else {
      continue;
    };
    let Said::Comment(CommentState::Sent { at }) = comment.said() else {
      continue;
    };
    let commit = lines.changed_by(thread);
    let words = outputs
      .iter()
      .find(|output| output.created_at >= *at && !output.text.trim().is_empty())
      .map(|output| output.text.trim().to_owned());
    let addressed = Said::Comment(CommentState::Addressed { commit });
    let changed = db.change_review_message(comment.id(), |said| {
      matches!(said, Said::Comment(CommentState::Sent { .. })).then(|| addressed.clone())
    })?;
    if let Some(words) = words.filter(|_| changed) {
      db.add_review_message(thread.id(), &ReviewMessage::agent(words, false, Timestamp::now()))?;
    }
  }
  Ok(())
}

/// What a thread's lines are read against: the run's diff up to each commit of its branch.
struct Lines<'a> {
  root: &'a Path,
  programs: &'a Programs,
  range: &'a RunRange,
  /// Where the run's branch is.
  head: Option<&'a Revision>,
}

impl Lines<'_> {
  /// The first commit after `thread`'s head, up to the run's head, that changed the thread's lines; `None` when none
  /// did, or when git cannot tell (the log says why).
  fn changed_by(&self, thread: &ReviewThread) -> Option<Revision> {
    let tip = self.head?;
    if tip == thread.head() {
      return None;
    }
    let commits = own_commits(&self.programs.runner(), Path::new(GIT), (self.root, thread.head(), tip))
      .inspect_err(
        |error| tracing::warn!(%error, thread = %thread.id(), "cannot list the commits after a thread's head"),
      )
      .ok()?;
    let base = own_start(self.root, self.programs, self.range, tip)
      .inspect_err(|error| tracing::warn!(%error, thread = %thread.id(), "cannot tell where the run's work starts"))
      .ok()?;
    commits.into_iter().rev().find_map(|commit| {
      let read = checkpointed_in(self.root, self.programs, &base, Some(&commit.revision))
        .inspect_err(
          |error| tracing::warn!(%error, commit = %commit.revision, "cannot read the run's diff at a commit"),
        )
        .ok()?;
      let Checkpointed::Read { changes, .. } = read else {
        // Larger than Orchester reads: whether the lines changed there cannot be told.
        return None;
      };
      let file = changes
        .diff
        .files()
        .iter()
        .find(|file| file.path() == thread.anchor().path());
      matches!(locate(thread.anchor(), file), Located::Outdated).then_some(commit.revision)
    })
  }
}
