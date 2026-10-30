//! The order of the person's inputs (`docs/design/browser.md`, "The person's input"). The window's requests run on
//! tasks of their own, and a left press first asks the page what lies under it, so a release could overtake its press.
//! Each input takes its place in its tab's line as it arrives and goes out when the one before it has gone out. It does
//! not wait for the one before to be answered: the browser keeps the order of what it is sent, and a click that opens a
//! dialog is not answered until the dialog is. Tabs do not share a line, so a page that does not answer holds up only
//! its own tab.

use std::collections::HashMap;

use orchester_ipc::browser::BrowserCommand;
use parking_lot::Mutex;
use tokio::sync::oneshot;

use crate::browser::cdp::TargetId;

/// The last input to arrive on each tab, while its tab is open.
#[derive(Default)]
pub(super) struct InputLines {
  last: Mutex<HashMap<TargetId, oneshot::Receiver<()>>>,
}

impl InputLines {
  /// The place of an input that just arrived on `tab`, behind the one that arrived before it.
  pub(super) fn join(&self, tab: TargetId) -> Turn {
    let (gone, after) = oneshot::channel();
    let before = self.last.lock().insert(tab, after);
    Turn {
      before,
      _gone: Some(gone),
    }
  }

  /// `tab` closed.
  pub(super) fn forget(&self, tab: &TargetId) {
    self.last.lock().remove(tab);
  }

  /// The browser starts again, with tabs of new ids.
  pub(super) fn clear(&self) {
    self.last.lock().clear();
  }
}

/// A command's place in its tab's line; a command that is no input has none and waits for nothing. Dropping it, once
/// its input has gone out or will not, lets the next input go.
#[derive(Default)]
pub(super) struct Turn {
  before: Option<oneshot::Receiver<()>>,
  _gone: Option<oneshot::Sender<()>>,
}

impl Turn {
  /// Waits until the input before this one has gone out.
  pub(super) async fn come(&mut self) {
    if let Some(before) = self.before.take() {
      // Nothing is ever sent: the channel closing is the signal.
      let _ = before.await;
    }
  }
}

/// One of the window's commands, in its place from the moment it arrived ([`ProjectBrowser::arrived`]), to be carried
/// out ([`ProjectBrowser::serve`]).
///
/// [`ProjectBrowser::arrived`]: crate::browser::ProjectBrowser::arrived
/// [`ProjectBrowser::serve`]: crate::browser::ProjectBrowser::serve
pub struct Arrived {
  pub(super) command: BrowserCommand,
  pub(super) turn: Turn,
}
