//! What the composer keeps of each task for as long as the app runs, beyond the task's page: the draft, so
//! leaving the task and coming back finds it; the words last sent and the key they are stored under until a
//! store succeeds, so words sent again after an unclear failure, or after the page closed during a send
//! whose end nobody saw, are not stored twice (changed words get a new key; a send that went through
//! forgets them itself, whether or not a page still waits for it); and the refused messages the person put
//! back into the composer, which offer nothing more.

use std::{collections::HashMap, sync::LazyLock};

use orchester_core::id::{IdempotencyKey, MessageId, TaskId};
use parking_lot::Mutex;

#[derive(Default)]
struct Kept {
  draft: String,
  attempt: Option<(String, IdempotencyKey)>,
  handled: Vec<MessageId>,
}

static KEPT: LazyLock<Mutex<HashMap<TaskId, Kept>>> = LazyLock::new(Mutex::default);

fn with<T>(task: TaskId, read: impl FnOnce(&mut Kept) -> T) -> T {
  read(KEPT.lock().entry(task).or_default())
}

pub(super) fn draft(task: TaskId) -> String {
  with(task, |kept| kept.draft.clone())
}

pub(super) fn set_draft(task: TaskId, text: &str) {
  with(task, |kept| text.clone_into(&mut kept.draft));
}

/// The key `text` of `task` is stored under: the one of the last attempt when the words did not change.
pub(super) fn key_for(task: TaskId, text: &str) -> IdempotencyKey {
  with(task, |kept| match &kept.attempt {
    Some((sent, key)) if sent == text => *key,
    _ => {
      let key = IdempotencyKey::generate();
      kept.attempt = Some((text.to_owned(), key));
      key
    }
  })
}

/// The words of `task` sent under `key` went through.
pub(super) fn forget(task: TaskId, key: IdempotencyKey) {
  with(task, |kept| {
    if kept.attempt.as_ref().is_some_and(|(_, sent)| *sent == key) {
      kept.attempt = None;
    }
  });
}

pub(super) fn handled(task: TaskId) -> Vec<MessageId> {
  with(task, |kept| kept.handled.clone())
}

/// The refused message `message` of `task` was put back into the composer.
pub(super) fn handle(task: TaskId, message: MessageId) {
  with(task, |kept| {
    if !kept.handled.contains(&message) {
      kept.handled.push(message);
    }
  });
}
