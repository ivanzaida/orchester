//! What the cards keep of each run for as long as the app runs, beyond the Conversation tab and the task's
//! page: what the person picked and typed on a question's card, so switching to the Summary and back, or
//! leaving the task, finds it; and what became of each answer given, recorded where the answer ends (on the
//! app's runtime), so an answer that finishes while no tab shows its card is known when one does. Kept by
//! run and card key. The runner's records take over once the run is read again.

use std::{
  collections::HashMap,
  sync::{Arc, LazyLock},
};

use orchester_core::{id::RunId, question::Question};
use orchester_workspace::tasks::question::Draft;
use parking_lot::Mutex;

use super::answers::Outcome;

#[derive(Default)]
struct Kept {
  /// What is picked, with each own answer as typed; `None` until the card was first shown.
  draft: Option<Draft>,
  outcome: Option<Outcome>,
}

/// What is kept, by run and card key.
type ByCard = HashMap<(RunId, Arc<str>), Kept>;

static KEPT: LazyLock<Mutex<ByCard>> = LazyLock::new(Mutex::default);

fn with<T>((run, key): (RunId, &Arc<str>), read: impl FnOnce(&mut Kept) -> T) -> T {
  read(KEPT.lock().entry((run, key.clone())).or_default())
}

/// What the person picked and typed on the card `key` of `run`, asking `questions`.
pub(super) fn draft(card: (RunId, &Arc<str>), questions: &[Question]) -> Draft {
  with(card, |kept| {
    kept.draft.get_or_insert_with(|| Draft::new(questions)).clone()
  })
}

/// Changes what is picked and typed on the card with `change`.
pub(super) fn change_draft(card: (RunId, &Arc<str>), questions: &[Question], change: impl FnOnce(&mut Draft)) {
  with(card, |kept| {
    change(kept.draft.get_or_insert_with(|| Draft::new(questions)))
  });
}

pub(super) fn outcome(card: (RunId, &Arc<str>)) -> Option<Outcome> {
  with(card, |kept| kept.outcome.clone())
}

pub(super) fn set_outcome(card: (RunId, &Arc<str>), outcome: Outcome) {
  with(card, |kept| kept.outcome = Some(outcome));
}
