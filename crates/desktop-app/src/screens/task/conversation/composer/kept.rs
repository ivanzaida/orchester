//! What the composer keeps of each task for as long as the app runs, beyond the task's page: the draft, so
//! leaving the task and coming back finds it; the words last sent, how, and the key they are stored under
//! until a store succeeds, so the same words sent the same way again after an unclear failure, or after the
//! page closed during a send whose end nobody saw, are not stored twice; and the refused messages the person
//! put back into the composer, which offer nothing more while their words are in the draft or once they were
//! sent. A send that went through clears its words from here itself, whether or not a page still shows the
//! task. A task with nothing kept has no entry.

use std::{collections::HashMap, sync::LazyLock};

use orchester_core::id::{IdempotencyKey, MessageId, TaskId};
use orchester_workspace::tasks::conversation::outgoing::{Action, Way};
use parking_lot::Mutex;

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

#[derive(Default)]
struct Kept {
  draft: String,
  attempt: Option<Attempt>,
  put_back: Vec<PutBack>,
}

impl Kept {
  fn is_empty(&self) -> bool {
    self.draft.is_empty() && self.attempt.is_none() && self.put_back.is_empty()
  }
}

static KEPT: LazyLock<Mutex<HashMap<TaskId, Kept>>> = LazyLock::new(Mutex::default);

/// Changes what is kept of `task`; an entry left with nothing goes.
fn change<T>(task: TaskId, change: impl FnOnce(&mut Kept) -> T) -> T {
  let mut all = KEPT.lock();
  let kept = all.entry(task).or_default();
  let changed = change(kept);
  if kept.is_empty() {
    all.remove(&task);
  }
  changed
}

fn read<T: Default>(task: TaskId, read: impl FnOnce(&Kept) -> T) -> T {
  KEPT.lock().get(&task).map(read).unwrap_or_default()
}

pub(super) fn draft(task: TaskId) -> String {
  read(task, |kept| kept.draft.clone())
}

/// The draft of `task` is `text` now: a message put back whose words are gone from it offers again,
/// unless they were sent.
pub(super) fn set_draft(task: TaskId, text: &str) {
  change(task, |kept| {
    text.clone_into(&mut kept.draft);
    kept.put_back.retain(|put| put.sent || text.contains(&put.text));
  });
}

/// The key `text` of `task`, sent `how`, is stored under: the one of the last attempt when neither the
/// words nor how they are sent changed.
pub(super) fn key_for(task: TaskId, text: &str, how: (Way, Action)) -> IdempotencyKey {
  change(task, |kept| match &kept.attempt {
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
  change(task, |kept| {
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
  read(task, |kept| kept.put_back.iter().map(|put| put.message).collect())
}

/// The refused message `message` of `task`, which says `text`, was put back into the composer.
pub(super) fn handle(task: TaskId, message: MessageId, text: &str) {
  change(task, |kept| {
    if !kept.put_back.iter().any(|put| put.message == message) {
      kept.put_back.push(PutBack {
        message,
        text: text.to_owned(),
        sent: false,
      });
    }
  });
}
