//! Saving the tabs whenever they change (design doc, "Tabs" › "Restore"): the latest state is
//! written to the application database off the UI thread, one write at a time; states that
//! change while a write runs are coalesced into the next.

use std::sync::Arc;

use parking_lot::Mutex;
use tokio::runtime::Handle;

use super::model::TabSet;
use crate::workspace::Workspace;

pub(super) struct Saver {
  workspace: Workspace,
  runtime: Handle,
  state: Arc<Mutex<State>>,
}

#[derive(Default)]
struct State {
  /// The text written last, or being written.
  written: Option<String>,
  /// The newest text waiting for the running write.
  next: Option<String>,
  writing: bool,
}

impl Saver {
  pub(super) fn new(workspace: Workspace, runtime: Handle) -> Self {
    Self {
      workspace,
      runtime,
      state: Arc::default(),
    }
  }

  /// Saves `set` unless it is what was saved last.
  pub(super) fn save(&self, set: &TabSet) {
    let text = set.saved().encode();
    {
      let mut state = self.state.lock();
      if state.next.as_ref().or(state.written.as_ref()) == Some(&text) {
        return;
      }
      state.next = Some(text);
      if state.writing {
        return;
      }
      state.writing = true;
    }
    let (workspace, state) = (self.workspace.clone(), self.state.clone());
    self.runtime.spawn_blocking(move || write_pending(&workspace, &state));
  }
}

/// Writes the waiting text until none is left.
fn write_pending(workspace: &Workspace, state: &Mutex<State>) {
  loop {
    let text = {
      let mut state = state.lock();
      let Some(text) = state.next.take() else {
        state.writing = false;
        return;
      };
      state.written = Some(text.clone());
      text
    };
    // Tabs that cannot be saved are only not restored next time; the log says why.
    if let Err(error) = workspace.save_tabs(&text) {
      tracing::error!(%error, "cannot save the window's tabs");
    }
  }
}
