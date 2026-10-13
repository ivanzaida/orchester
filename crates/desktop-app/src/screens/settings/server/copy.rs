//! Copy of the token, or of a command with the token: it goes on the clipboard, which any program running as the
//! person can read, so Orchester takes it off again after a minute if the clipboard still holds what it put there
//! (decided 2026-10-13). What was copied is kept in memory until then, wiped when dropped.

use std::time::Duration;

use lurq::clipboard::{clear_clipboard, copy_to_clipboard, read_from_clipboard};
use tokio::runtime::Handle;
use zeroize::Zeroizing;

/// How long the clipboard keeps a copied token.
const KEEP: Duration = Duration::from_secs(60);

/// Puts `text` on the clipboard and takes it off again after [`KEEP`] on `runtime`, unless something else was
/// copied meanwhile; `true` once it is there.
pub fn copy_for_a_minute(runtime: &Handle, text: String) -> bool {
  let text = Zeroizing::new(text);
  if !copy_to_clipboard(text.as_str()) {
    return false;
  }
  drop(runtime.spawn(async move {
    tokio::time::sleep(KEEP).await;
    let cleared = tokio::task::spawn_blocking(move || {
      let held = read_from_clipboard().map(Zeroizing::new);
      // Something else was copied meanwhile: it is the person's, and stays.
      held.is_none_or(|held| *held != *text) || clear_clipboard()
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
