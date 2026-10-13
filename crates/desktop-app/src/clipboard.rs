//! Copy of the app's MCP token, or of a command with the token: it goes on the clipboard, which any program running as
//! the person can read, so Orchester takes it off again after a minute if the clipboard still holds what it put there
//! (decided 2026-10-13), and when the app closes before then ([`clear_at_exit`]). What was copied last is kept in
//! memory until then, wiped when dropped. It is kept for the process, as the clipboard is the process's: the window
//! that copied it may be gone by the time the app closes.

use std::time::Duration;

use lurq::clipboard::{clear_clipboard, copy_to_clipboard, read_from_clipboard};
use parking_lot::{Mutex, const_mutex};
use tokio::runtime::Handle;
use zeroize::Zeroizing;

/// How long the clipboard keeps a copied token.
const KEEP: Duration = Duration::from_secs(60);

/// The text copied last, until its minute is up or the app closes.
static COPIED: Mutex<Option<Zeroizing<String>>> = const_mutex(None);

/// Puts `text` on the clipboard and takes it off again after [`KEEP`] on `runtime`, unless something else was
/// copied meanwhile; `true` once it is there.
pub fn copy_for_a_minute(runtime: &Handle, text: String) -> bool {
  let text = Zeroizing::new(text);
  if !copy_to_clipboard(text.as_str()) {
    return false;
  }
  *COPIED.lock() = Some(text.clone());
  drop(runtime.spawn(async move {
    tokio::time::sleep(KEEP).await;
    let cleared = tokio::task::spawn_blocking(move || {
      let mut copied = COPIED.lock();
      if copied.as_ref().is_some_and(|copied| **copied == *text) {
        *copied = None;
      }
      drop(copied);
      clear_if_held(&text)
    })
    .await;
    match cleared {
      Ok(true) => {}
      Ok(false) => tracing::warn!("the copied MCP token could not be cleared from the clipboard"),
      Err(error) => tracing::error!(%error, "clearing the copied MCP token did not finish"),
    }
  }));
  true
}

/// Takes the text copied last off the clipboard if it is still there: the app closes before its minute is up.
/// Blocks; called once the window has closed.
pub fn clear_at_exit() {
  let Some(text) = COPIED.lock().take() else {
    return;
  };
  if !clear_if_held(&text) {
    tracing::warn!("the copied MCP token could not be cleared from the clipboard as the app closed");
  }
}

/// Clears the clipboard if it holds `text`; something else copied meanwhile is the person's, and stays. `true`
/// unless clearing failed.
fn clear_if_held(text: &str) -> bool {
  let held = read_from_clipboard().map(Zeroizing::new);
  held.is_none_or(|held| held.as_str() != text) || clear_clipboard()
}
