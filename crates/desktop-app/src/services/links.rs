//! Where the app opens web pages (*Create an account*, *Forgot password*, *Open server admin*): the system browser.
//! A QA copy or a test started with `ORCHESTER_APP_RECORD_LINKS` set only records what it would have opened, so an
//! unattended run never opens a browser on the person's desktop; the record is what a test checks.

use std::{env, io, sync::Arc};

use parking_lot::Mutex;

/// The environment variable that makes the app record links instead of opening them.
pub const RECORD_VARIABLE: &str = "ORCHESTER_APP_RECORD_LINKS";

#[derive(Clone)]
pub enum Links {
  /// The default browser, through `orchester_exec::external::open_url` (https only).
  System,
  /// Records each address in order, and opens nothing.
  Recorded(Arc<Mutex<Vec<String>>>),
}

impl Links {
  /// The system browser, or a record when [`RECORD_VARIABLE`] is set.
  pub fn from_env() -> Self {
    if env::var_os(RECORD_VARIABLE).is_some() {
      Self::Recorded(Arc::default())
    } else {
      Self::System
    }
  }

  /// Opens `url` in the browser, or records it.
  pub fn open(&self, url: &str) -> io::Result<()> {
    match self {
      Self::System => orchester_exec::external::open_url(url),
      Self::Recorded(opened) => {
        tracing::info!(url, "a link was recorded instead of opened");
        opened.lock().push(url.to_owned());
        Ok(())
      }
    }
  }

  /// The addresses recorded so far; none for the system browser.
  pub fn recorded(&self) -> Vec<String> {
    match self {
      Self::System => Vec::new(),
      Self::Recorded(opened) => opened.lock().clone(),
    }
  }
}
