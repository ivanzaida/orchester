//! Which Chromium-based browser this computer has, as the Browser window says it (`07 · Browser not found` `YEv2c`):
//! `orchester_exec::browser::find`, run off the UI thread. A test or a QA copy can make the session look nowhere
//! instead ([`BrowserLookup::look_nowhere`]), so the window's *Browser not found* can be reached on a computer that has
//! a browser; the runner still finds its own.

use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

use orchester_exec::browser::{BrowserProgram, BrowserUnavailable, find};

/// Where the session looks for the browser. Clones share it.
#[derive(Clone, Default)]
pub struct BrowserLookup {
  nowhere: Arc<AtomicBool>,
}

impl BrowserLookup {
  /// The browser found, or why there is none. Blocks: on Windows it reads the registry.
  pub fn find(&self) -> Result<BrowserProgram, BrowserUnavailable> {
    if self.nowhere.load(Ordering::Relaxed) {
      return Err(BrowserUnavailable::new(
        "no Chromium-based browser is installed: this session looks nowhere",
      ));
    }
    find()
  }

  /// From now on this session finds no browser.
  pub fn look_nowhere(&self) {
    self.nowhere.store(true, Ordering::Relaxed);
  }
}
