//! Hand-offs to the project's runner cut off part-way. Comments are kept sending, and a question asked, before they
//! are handed to the runner; this process keeps which hand-offs are under way ([`Holding`]). One that is not under way
//! any more was cut off, as when the app ended meanwhile, and nothing else would move it on, so reading the run's
//! threads recovers it ([`threads`]): comments a later attempt of the run returned to Implement with were taken, and
//! are sent; the others wait unsent again, lost. A question the runner takes is answering at once, so one still asked
//! was never taken: it is *Not asked*, lost, and may be asked again. Blocks on SQLite.

use std::collections::BTreeSet;

use orchester_core::{
  id::{ReviewMessageId, RunId},
  review::{CommentState, QuestionState, ReviewFailure, ReviewThread, Said},
  run::{AttemptReason, Run},
  store::RunStore,
};
use orchester_store::ProjectDb;
use parking_lot::Mutex;

use super::ReviewRefusal;
use crate::tasks::detail::run_store_error;

/// The messages whose hand-off to the runner this process has under way.
static UNDER_WAY: Mutex<BTreeSet<ReviewMessageId>> = Mutex::new(BTreeSet::new());

/// Messages whose hand-off is under way until this is dropped, unless it is kept for the hand-off ([`Holding::keep`]).
#[must_use]
pub(super) struct Holding(Vec<ReviewMessageId>);

impl Holding {
  /// Holds `messages` under way, before they are kept sending or asked.
  pub(super) fn hold(messages: Vec<ReviewMessageId>) -> Self {
    UNDER_WAY.lock().extend(messages.iter().copied());
    Self(messages)
  }

  /// Takes over `messages`, held already, until the hand-off ends.
  pub(super) const fn held(messages: Vec<ReviewMessageId>) -> Self {
    Self(messages)
  }

  /// Leaves `kept` of the messages held, for the hand-off that takes them over ([`Holding::held`]); the others are
  /// released now.
  pub(super) fn keep(mut self, kept: &[ReviewMessageId]) {
    self.0.retain(|message| !kept.contains(message));
  }
}

impl Drop for Holding {
  fn drop(&mut self) {
    let mut under_way = UNDER_WAY.lock();
    for message in &self.0 {
      under_way.remove(message);
    }
  }
}

/// `run`'s review threads in `db`, the hand-offs cut off part-way recovered first.
pub(super) fn threads(db: &mut ProjectDb, run: RunId) -> Result<Vec<ReviewThread>, ReviewRefusal> {
  let threads = db.review_threads(run)?;
  let stale: Vec<(ReviewMessageId, Said)> = {
    let under_way = UNDER_WAY.lock();
    threads
      .iter()
      .flat_map(ReviewThread::messages)
      .filter(|message| !under_way.contains(&message.id()))
      .filter(|message| {
        matches!(
          message.said(),
          Said::Comment(CommentState::Sending { .. }) | Said::Question(QuestionState::Asked)
        )
      })
      .map(|message| (message.id(), message.said().clone()))
      .collect()
  };
  if stale.is_empty() {
    return Ok(threads);
  }
  let stored = db.run(run).map_err(run_store_error)?.ok_or(ReviewRefusal::NotFound)?;
  for (message, said) in stale {
    let recovered = recovered(&stored, &said);
    tracing::info!(%run, %message, ?recovered, "a review hand-off was cut off part-way");
    db.change_review_message(message, |now| (*now == said).then(|| recovered.clone()))?;
  }
  Ok(db.review_threads(run)?)
}

/// What `said`, cut off part-way while `run` stands as it does, becomes.
fn recovered(run: &Run, said: &Said) -> Said {
  match said {
    Said::Comment(CommentState::Sending { since, attempts }) => {
      let taken = run
        .attempts()
        .iter()
        .skip(*attempts)
        .any(|attempt| matches!(attempt.reason(), AttemptReason::ReturnedAfterEnd { .. }));
      Said::Comment(if taken {
        CommentState::Sent { at: *since }
      } else {
        CommentState::NotSent {
          failure: Some(ReviewFailure::Lost),
        }
      })
    }
    _ => Said::Question(QuestionState::NotAsked {
      failure: ReviewFailure::Lost,
    }),
  }
}
