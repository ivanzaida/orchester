use std::sync::Arc;

use parking_lot::Mutex;

use super::tools::ToolError;
use crate::navigation::Navigation;

/// The window's navigation for the app's MCP tools, which are registered before the window's root
/// creates it; the root attaches it once.
#[derive(Clone, Default)]
pub struct WindowLink(Arc<Mutex<Option<Navigation>>>);

impl WindowLink {
  pub fn attach(&self, navigation: Navigation) {
    *self.0.lock() = Some(navigation);
  }

  /// The navigation, or [`ToolError::NotReady`] before the root attached it.
  pub fn navigation(&self) -> Result<Navigation, ToolError> {
    self.0.lock().clone().ok_or(ToolError::NotReady)
  }
}

/// Links are compared by identity: the root's props hold the one link of the window.
impl PartialEq for WindowLink {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}
