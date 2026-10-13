//! The app's MCP server as the window sees it: its status as it changes, and the server itself for Settings to
//! change it, when the window has one.

use tokio::sync::watch;

use super::{control::AppMcp, status::McpStatus};

#[derive(Clone)]
pub struct McpLink {
  status: watch::Receiver<McpStatus>,
  server: Option<AppMcp>,
}

impl McpLink {
  pub fn new(server: &AppMcp) -> Self {
    Self {
      status: server.subscribe(),
      server: Some(server.clone()),
    }
  }

  /// A window without a server, e.g. in a test: `status` stays as it is.
  pub fn without_server(status: McpStatus) -> Self {
    Self {
      status: watch::channel(status).1,
      server: None,
    }
  }

  pub fn status(&self) -> McpStatus {
    self.status.borrow().clone()
  }

  /// The status now and each change after.
  pub fn changes(&self) -> watch::Receiver<McpStatus> {
    self.status.clone()
  }

  pub const fn server(&self) -> Option<&AppMcp> {
    self.server.as_ref()
  }
}

/// Links are compared by the status they follow: the root's props hold the window's one link.
impl PartialEq for McpLink {
  fn eq(&self, other: &Self) -> bool {
    self.status.same_channel(&other.status)
  }
}

impl lurq::app::component::DevtoolsInspectable for McpLink {}
