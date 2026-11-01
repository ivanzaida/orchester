//! A state set for testing (the app's MCP tool `set_update_state`, in debug builds only): the window and the MCP
//! server show it instead of the real one, so a QA copy reaches every update state on screen. It is kept in memory
//! only, never in a database, and is gone at the next start. It never checks, downloads or installs anything:
//! [`AppUpdates::install`] refuses while it shows. Behind it the real state goes on as before, and
//! [`AppUpdates::show_real_state`] shows it again.

use super::{AppUpdates, UpdateState, install::JustUpdated};

/// What shows besides the state set for testing.
pub(super) struct SetForTesting {
  /// The update shown as finished at this start, in place of the real one.
  pub(super) just_updated: Option<JustUpdated>,
}

impl AppUpdates {
  /// Shows `state`, with `just_updated` as the update this start finished, instead of the real ones until
  /// [`Self::show_real_state`].
  pub fn set_for_testing(&self, state: UpdateState, just_updated: Option<JustUpdated>) {
    let mut testing = self.inner.testing.lock();
    *testing = Some(SetForTesting { just_updated });
    // Every change wakes what follows the state, so a new `just_updated` shows even with the same state.
    self.inner.shown.send_replace(state);
  }

  /// Shows the real state and the real update this start finished again.
  pub fn show_real_state(&self) {
    let mut testing = self.inner.testing.lock();
    *testing = None;
    self.inner.shown.send_replace(self.inner.state.borrow().clone());
  }

  /// Whether a state set for testing shows instead of the real one.
  pub fn is_set_for_testing(&self) -> bool {
    self.inner.testing.lock().is_some()
  }

  /// Shows the real state after it changed, unless a state set for testing shows instead.
  pub(super) fn show_real(&self) {
    let testing = self.inner.testing.lock();
    if testing.is_none() {
      self.inner.shown.send_replace(self.inner.state.borrow().clone());
    }
  }
}
