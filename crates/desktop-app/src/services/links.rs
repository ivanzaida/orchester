//! Where the app opens web pages (*Create an account*, *Forgot password*, *Open server admin*): the system browser.
//! A QA copy started with `ORCHESTER_APP_RECORD_LINKS` set, or a test that asks for it ([`Links::record_instead`]),
//! only records what it would have opened, so an unattended run never opens a browser on the person's desktop; the
//! record is what a test checks.

use std::{env, io, sync::Arc};

use parking_lot::Mutex;

/// The environment variable that makes the app record links instead of opening them.
pub const RECORD_VARIABLE: &str = "ORCHESTER_APP_RECORD_LINKS";

/// The session's links. Clones share whether they record and what they recorded.
#[derive(Clone, Default)]
pub struct Links {
  /// `None` while pages open in the browser; the addresses recorded so far otherwise.
  recorded: Arc<Mutex<Option<Vec<String>>>>,
}

impl Links {
  /// The system browser, or a record when [`RECORD_VARIABLE`] is set.
  pub fn from_env() -> Self {
    let links = Self::default();
    if env::var_os(RECORD_VARIABLE).is_some() {
      links.record_instead();
    }
    links
  }

  /// From now on this session records addresses instead of opening them.
  pub fn record_instead(&self) {
    self.recorded.lock().get_or_insert_with(Vec::new);
  }

  /// Opens `url` in the browser (`orchester_exec::external::open_url`, https only), or records it.
  pub fn open(&self, url: &str) -> io::Result<()> {
    if let Some(recorded) = self.recorded.lock().as_mut() {
      tracing::info!(url, "a link was recorded instead of opened");
      recorded.push(url.to_owned());
      return Ok(());
    }
    orchester_exec::external::open_url(url)
  }

  /// The addresses recorded so far; none while pages open in the browser.
  pub fn recorded(&self) -> Vec<String> {
    self.recorded.lock().clone().unwrap_or_default()
  }
}
