//! The messages the executor and its coordinator exchange, as JSON, in the runner's own tests
//! (`docs/architecture/remote-runners.md`, rule 4): each one crosses as text and must arrive unchanged, as it would in
//! another process. A test that [`watch`]es them reads every text sent meanwhile ([`Watch::texts`]), so it can check
//! that no secret value is part of any of them (ORC-673).

use std::sync::{
  Mutex,
  atomic::{AtomicUsize, Ordering},
};

/// How many watches are open: the texts are kept only while one is.
static WATCHES: AtomicUsize = AtomicUsize::new(0);
/// The texts sent while a watch was open, by every run of the process.
static SENT: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Keeps the messages sent from now on until it is dropped.
pub struct Watch {
  from: usize,
}

/// Starts keeping the messages sent.
pub fn watch() -> Watch {
  WATCHES.fetch_add(1, Ordering::SeqCst);
  Watch { from: sent().len() }
}

impl Watch {
  /// The texts sent since the watch started, by every run of the process.
  pub fn texts(&self) -> Vec<String> {
    sent().get(self.from..).map(<[String]>::to_vec).unwrap_or_default()
  }
}

impl Drop for Watch {
  fn drop(&mut self) {
    if WATCHES.fetch_sub(1, Ordering::SeqCst) == 1 {
      sent().clear();
    }
  }
}

fn sent() -> std::sync::MutexGuard<'static, Vec<String>> {
  SENT.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// `message` as it is sent: written as JSON, kept while a watch is open, and read back.
fn send<T: serde::Serialize + serde::de::DeserializeOwned>(message: &T) -> T {
  let text = serde_json::to_string(message).expect("a message over the link serializes");
  let received = serde_json::from_str(&text).expect("a message over the link deserializes");
  if WATCHES.load(Ordering::SeqCst) > 0 {
    sent().push(text);
  }
  received
}

/// Sends `message` and checks that it arrives unchanged.
pub(in crate::execution) fn survives_sending<T>(message: &T)
where
  T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
  let received = send(message);
  assert_eq!(&received, message, "the message survives being sent");
}

/// Sends `message`, whose arrival is not compared.
pub(in crate::execution) fn sending<T: serde::Serialize + serde::de::DeserializeOwned>(message: &T) {
  send(message);
}
