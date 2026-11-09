//! What *Since your review* shows on the review page (Review v2, screen 05 `e6KEO`): the run's diff from the head the
//! person reviewed (the page's `reviewed`) to where its branch is now (its `review.head`), in
//! the shape of the page's whole diff, so the page shows either. The threads keep their anchors and states: a thread
//! whose lines a later commit changed is outdated as [`super::read`] tells, whichever diff is shown. A branch that was
//! rebased or reset after the reviewed head holds no diff since it, only a reason. Blocks on SQLite and git.

use std::path::Path;

use orchester_core::{integration::is_ancestor, revision::Revision};
use orchester_exec::programs::Programs;
use thiserror::Error;

use crate::{
  messages::Message,
  tasks::changes::{ChangesError, Checkpointed, GIT, checkpointed_in},
};

/// Why there is no diff since the reviewed head.
#[derive(Debug, Error)]
pub enum SinceReviewError {
  /// No head of the run was reviewed: there is nothing to compare its branch with.
  #[error("no head of the run was reviewed")]
  NotReviewed,
  /// The branch was rebased or reset after `reviewed`, so `head` does not hold it: what changed since the review
  /// cannot be told apart from what the rewrite changed.
  #[error("the reviewed head {reviewed} is not in the history of the branch's head {head}")]
  Rewritten { reviewed: Revision, head: Revision },
  /// Git or the project's data could not be read.
  #[error(transparent)]
  Changes(#[from] ChangesError),
}

impl SinceReviewError {
  /// The code automation reads.
  pub const fn code(&self) -> &'static str {
    match self {
      Self::NotReviewed => "not_reviewed",
      Self::Rewritten { .. } => "rewritten_since_review",
      Self::Changes(_) => "changes",
    }
  }

  /// What a person reads.
  pub fn message(&self) -> Message {
    match self {
      Self::NotReviewed => Message::new("review.since.notReviewed"),
      Self::Rewritten { .. } => Message::new("review.since.rewritten"),
      Self::Changes(error) => error.message(),
    }
  }
}

/// The diff of the repository at `root` from `reviewed`, the head the person reviewed, to `head`, where the run's
/// branch is now (`None` before its first checkpoint: nothing to show), read with `programs` as the page's whole diff
/// is. It is empty while the branch is still at `reviewed`.
pub fn diff_since_review(
  (root, programs): (&Path, &Programs),
  (reviewed, head): (Option<&Revision>, Option<&Revision>),
) -> Result<Checkpointed, SinceReviewError> {
  let Some(head) = head else {
    return Ok(Checkpointed::NoCheckpoint);
  };
  let reviewed = reviewed.ok_or(SinceReviewError::NotReviewed)?;
  let held = is_ancestor(&programs.runner(), Path::new(GIT), (root, reviewed, head)).map_err(ChangesError::Log)?;
  if !held {
    return Err(SinceReviewError::Rewritten {
      reviewed: reviewed.clone(),
      head: head.clone(),
    });
  }
  Ok(checkpointed_in(root, programs, reviewed, Some(head))?)
}
