//! What the composer keeps of each task for as long as the app runs, beyond the task's page: the draft, so
//! leaving the task and coming back finds it; the words last sent, how, and the key they are stored under
//! until a store succeeds, so the same words sent the same way again after an unclear failure, or after the
//! page closed during a send whose end nobody saw, are not stored twice; and the refused messages the person
//! put back into the composer, which offer nothing more while their words are in the draft or once they were
//! sent; and the send under way, so a page opened again while it runs shows it busy until it ends. A send
//! that went through clears its words from here itself, whether or not a page still shows the task. A task
//! with nothing kept has no entry.

use std::sync::atomic::{AtomicU64, Ordering};

use orchester_core::id::{IdempotencyKey, MessageId, TaskId};
use orchester_workspace::tasks::conversation::outgoing::{Action, Way};
use tokio::sync::watch;

use crate::screens::kept::{ByKey, Entry};

/// Words sent, how, and the key they are stored under.
struct Attempt {
  text: String,
  how: (Way, Action),
  key: IdempotencyKey,
}

/// A refused message put back into the composer: its words, and whether they were sent since.
struct PutBack {
  message: MessageId,
  text: String,
  sent: bool,
}

/// A send under way: which one, and when it ends.
#[derive(Clone)]
pub(super) struct Flight {
  pub(super) generation: u64,
  pub(super) ended: watch::Receiver<bool>,
}

#[derive(Default)]
struct Kept {
  draft: String,
  attempt: Option<Attempt>,
  put_back: Vec<PutBack>,
  flight: Option<Flight>,
}

impl Entry for Kept {
  fn is_empty(&self) -> bool {
    self.draft.is_empty() && self.attempt.is_none() && self.put_back.is_empty() && self.flight.is_none()
  }
}

static GENERATION: AtomicU64 = AtomicU64::new(1);

static KEPT: ByKey<TaskId, Kept> = ByKey::new();

pub(super) fn draft(task: TaskId) -> String {
  KEPT.read(task, |kept| kept.draft.clone())
}

/// The draft of `task` is `text` now: a message put back whose words are gone from it offers again,
/// unless they were sent.
pub(super) fn set_draft(task: TaskId, text: &str) {
  KEPT.change(task, |kept| {
    text.clone_into(&mut kept.draft);
    kept.put_back.retain(|put| put.sent || text.contains(&put.text));
  });
}

/// The key `text` of `task`, sent `how`, is stored under: the one of the last attempt when neither the
/// words nor how they are sent changed.
pub(super) fn key_for(task: TaskId, text: &str, how: (Way, Action)) -> IdempotencyKey {
  KEPT.change(task, |kept| match &kept.attempt {
    Some(attempt) if attempt.text == text && attempt.how == how => attempt.key,
    _ => {
      let key = IdempotencyKey::generate();
      kept.attempt = Some(Attempt {
        text: text.to_owned(),
        how,
        key,
      });
      key
    }
  })
}

/// `text` of `task`, sent under `key`, went through: its attempt goes, the draft empties unless it holds
/// other words by now, and messages put back that it holds were sent.
pub(super) fn stored(task: TaskId, key: IdempotencyKey, text: &str) {
  KEPT.change(task, |kept| {
    if kept.attempt.as_ref().is_some_and(|attempt| attempt.key == key) {
      kept.attempt = None;
    }
    if kept.draft == text {
      kept.draft.clear();
    }
    for put in &mut kept.put_back {
      put.sent |= text.contains(&put.text);
    }
  });
}

/// The refused messages of `task` put back into the composer that offer nothing more.
pub(super) fn handled(task: TaskId) -> Vec<MessageId> {
  KEPT.read(task, |kept| kept.put_back.iter().map(|put| put.message).collect())
}

/// The refused message `message` of `task`, which says `text`, was put back into the composer.
pub(super) fn handle(task: TaskId, message: MessageId, text: &str) {
  KEPT.change(task, |kept| {
    if !kept.put_back.iter().any(|put| put.message == message) {
      kept.put_back.push(PutBack {
        message,
        text: text.to_owned(),
        sent: false,
      });
    }
  });
}

/// A send of `task` starts: the generation it ends with ([`end`]), and what tells its end.
pub(super) fn begin(task: TaskId) -> (u64, watch::Sender<bool>) {
  let generation = GENERATION.fetch_add(1, Ordering::Relaxed);
  let (ended, receiver) = watch::channel(false);
  KEPT.change(task, |kept| {
    kept.flight = Some(Flight {
      generation,
      ended: receiver,
    });
  });
  (generation, ended)
}

/// The send `generation` of `task` ended, however it ended.
pub(super) fn end(task: TaskId, (generation, ended): (u64, watch::Sender<bool>)) {
  KEPT.change(task, |kept| {
    if kept
      .flight
      .as_ref()
      .is_some_and(|flight| flight.generation == generation)
    {
      kept.flight = None;
    }
  });
  // Nobody may wait for it any more: a page that closed dropped its receiver.
  ended.send_replace(true);
}

/// The send of `task` under way, if one is.
pub(super) fn flight(task: TaskId) -> Option<Flight> {
  KEPT.read(task, |kept| kept.flight.clone())
}
