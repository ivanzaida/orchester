//! The *Browser* window's side of the projects' runners: the window's state they fill ([`BrowserWindow`]) and its
//! requests to a project's browser ([`Command::Browser`]). The window shows only projects whose runner reported a
//! browser, so its requests start no runner.

use std::path::Path;

use orchester_ipc::{
  Command, Reply,
  browser::{BrowserCommand, BrowserReply},
};
use tokio::sync::oneshot;

use super::{RequestError, RunRequestError, Runners};
use crate::browser::BrowserWindow;

impl Runners {
  /// The *Browser* window's state, which every project's runner fills once it attached.
  pub fn browser_window(&self) -> BrowserWindow {
    self.inner.shared.browser.clone()
  }

  /// Sends the window's `command` to the browser of the project at `root`; the answer once it was carried out. The
  /// command goes out at this call, before the future is awaited, so the window's requests reach the runner in the
  /// order of these calls.
  pub fn browser(
    &self,
    root: &Path,
    command: BrowserCommand,
  ) -> impl Future<Output = Result<BrowserReply, RunRequestError>> + Send + use<> {
    let answer = self.browser_request(root, command);
    async move {
      match answer?.await.unwrap_or(Err(RequestError::Lost))? {
        Reply::Browser(reply) => Ok(reply),
        Reply::Rejected(rejection) => Err(RunRequestError::Rejected(rejection)),
        other => {
          tracing::error!(?other, "unexpected reply to a Browser request");
          Err(RunRequestError::Request(RequestError::Lost))
        }
      }
    }
  }

  fn browser_request(
    &self,
    root: &Path,
    command: BrowserCommand,
  ) -> Result<oneshot::Receiver<Result<Reply, RequestError>>, RequestError> {
    #[cfg(feature = "test-support")]
    let command = match self.inner.shared.browser.record(root, command) {
      Ok(answer) => return Ok(answer),
      Err(command) => command,
    };
    self.request(Some(root), Command::Browser(command))
  }
}
