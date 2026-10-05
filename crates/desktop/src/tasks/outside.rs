//! Changes of tasks made outside the screens, by automation through Orchester's MCP server: the
//! screens read the project's tasks again after each, as they do after their own saves.

use std::sync::Arc;

use tokio::sync::watch;

/// Counts the changes of tasks made outside the screens.
#[derive(Clone)]
pub struct OutsideChanges(Arc<watch::Sender<u64>>);

impl Default for OutsideChanges {
  fn default() -> Self {
    Self(Arc::new(watch::Sender::new(0)))
  }
}

impl PartialEq for OutsideChanges {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl lurq::app::component::DevtoolsInspectable for OutsideChanges {}

impl OutsideChanges {
  /// A task was saved outside the screens.
  pub fn record(&self) {
    self.0.send_modify(|count| *count += 1);
  }

  pub fn subscribe(&self) -> watch::Receiver<u64> {
    self.0.subscribe()
  }
}
