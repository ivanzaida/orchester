//! The *Browser* window's side of the projects' runners: the window's state they fill ([`BrowserWindow`]) and its
//! requests to a project's browser ([`Command::Browser`]). The window shows only projects whose runner reported a
//! browser, so its requests start no runner.

use std::path::Path;

use orchester_ipc::{
  Command, Reply,
  browser::{BrowserCommand, BrowserReply},
};

use super::{RequestError, RunRequestError, Runners};
use crate::browser::BrowserWindow;

impl Runners {
  /// The *Browser* window's state, which every project's runner fills once it attached.
  pub fn browser_window(&self) -> BrowserWindow {
    self.inner.shared.browser.clone()
  }

  /// Sends the window's `command` to the browser of the project at `root`; the answer once it was carried out.
  pub async fn browser(&self, root: &Path, command: BrowserCommand) -> Result<BrowserReply, RunRequestError> {
    #[cfg(feature = "test-support")]
    let Err(command) = self.inner.shared.browser.record(root, command) else {
      return Ok(BrowserReply::Done);
    };
    match self.ask(Some(root), Command::Browser(command)).await? {
      Reply::Browser(reply) => Ok(reply),
      Reply::Rejected(rejection) => Err(RunRequestError::Rejected(rejection)),
      other => {
        tracing::error!(?other, "unexpected reply to a Browser request");
        Err(RunRequestError::Request(RequestError::Lost))
      }
    }
  }
}
