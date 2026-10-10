//! The close flow of a window (`docs/architecture/storage.md`, "Lifecycle"; the closing panel of layer
//! "App lifecycle" `Ob1P7` in `data/orchester2.pen`). Orchester has no background mode: a close with no
//! agent at work stops the runners at once and closes; with agents at work the close is held while the
//! person chooses to wait for the agents to save, to quit now, or to cancel. The window's close hook,
//! which hands over how to let the window close, and the panel are each app's; what a close leads to
//! and how the session ends are here.

use std::{
  sync::Arc,
  time::{Duration, SystemTime},
};

use orchester_store::SessionEnd;
use parking_lot::Mutex;
use tokio::{runtime::Handle, sync::watch, task::JoinHandle};

use super::{CloseKind, ClosePlan, close_plan, session_end};
use crate::runner::{ActiveWork, QUIET_STOP_TIMEOUT, Runners, ShutdownOutcome};

/// Lets the window close; called once the close flow decided it may.
pub type Proceed = Box<dyn FnOnce() + Send>;

/// Where a close stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClosePhase {
  Open,
  /// The close is held while the person decides; the runs with an agent at work.
  Confirming(Arc<[ActiveWork]>),
  /// "Wait for agents to save" was chosen at `since`: the runners stop at checkpoints, with no
  /// deadline, and the window closes once they stopped.
  Waiting {
    work: Arc<[ActiveWork]>,
    since: SystemTime,
  },
  /// Nothing was at work: the runners stop at once and the window closes.
  Stopping,
}

impl ClosePhase {
  /// Whether the closing panel takes the window's body: the person decides or waits.
  pub const fn holds_window(&self) -> bool {
    matches!(self, Self::Confirming(_) | Self::Waiting { .. })
  }
}

/// The window's close flow; clones share it.
#[derive(Clone)]
pub struct CloseFlow {
  inner: Arc<Inner>,
}

struct Inner {
  runners: Runners,
  runtime: Handle,
  phase: watch::Sender<ClosePhase>,
  /// The held close; whoever takes it decides how the session ends.
  request: Mutex<Option<Proceed>>,
  end: Mutex<Option<SessionEnd>>,
  stopping: Mutex<Option<JoinHandle<()>>>,
}

impl PartialEq for CloseFlow {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.inner, &other.inner)
  }
}

impl CloseFlow {
  pub fn new(runners: Runners, runtime: Handle) -> Self {
    Self {
      inner: Arc::new(Inner {
        runners,
        runtime,
        phase: watch::Sender::new(ClosePhase::Open),
        request: Mutex::new(None),
        end: Mutex::new(None),
        stopping: Mutex::new(None),
      }),
    }
  }

  /// The phase, whenever it changes.
  pub fn subscribe(&self) -> watch::Receiver<ClosePhase> {
    self.inner.phase.subscribe()
  }

  pub fn phase(&self) -> ClosePhase {
    self.inner.phase.borrow().clone()
  }

  /// The window asks to close; `proceed` lets it. A request while a close is under way is dropped,
  /// which keeps the window open until that close finishes: the window's × and Alt+F4 / Cmd+Q do
  /// nothing more while the panel is shown.
  pub fn request(&self, proceed: impl FnOnce() + Send + 'static) {
    if self.phase() != ClosePhase::Open {
      return;
    }
    *self.inner.request.lock() = Some(Box::new(proceed));
    match close_plan(self.inner.runners.active_work()) {
      ClosePlan::StopQuietly => {
        self.inner.phase.send_replace(ClosePhase::Stopping);
        self.stop_then_close(CloseKind::Quiet, Some(QUIET_STOP_TIMEOUT));
      }
      ClosePlan::Confirm(work) => {
        self.inner.phase.send_replace(ClosePhase::Confirming(work));
      }
    }
  }

  /// "Wait for agents to save": no new steps or tool calls start, every agent goes on to its next
  /// checkpoint, the runs are paused, then the window closes. No deadline: Quit now stays offered.
  pub fn wait_for_agents(&self) {
    let ClosePhase::Confirming(work) = self.phase() else {
      return;
    };
    self.inner.phase.send_replace(ClosePhase::Waiting {
      work,
      since: SystemTime::now(),
    });
    self.stop_then_close(CloseKind::WaitForAgents, None);
  }

  /// "Quit now": the runners and their agents are ended at once, without a checkpoint, and the
  /// window closes.
  pub fn quit_now(&self) {
    if !self.phase().holds_window() {
      return;
    }
    if let Some(stopping) = self.inner.stopping.lock().take() {
      stopping.abort();
    }
    self.inner.runners.terminate();
    self.close(CloseKind::QuitNow, ShutdownOutcome::Unanswered);
  }

  /// Keeps the window open, unchanged; only while the person decides: once the runners stopped
  /// starting new work, cancelling would leave the runs paused without closing.
  pub fn cancel(&self) {
    if !matches!(self.phase(), ClosePhase::Confirming(_)) {
      return;
    }
    drop(self.inner.request.lock().take());
    self.inner.phase.send_replace(ClosePhase::Open);
  }

  fn stop_then_close(&self, kind: CloseKind, deadline: Option<Duration>) {
    let flow = self.clone();
    let task = self.inner.runtime.spawn(async move {
      let outcome = flow.inner.runners.shutdown(deadline).await;
      flow.close(kind, outcome);
    });
    *self.inner.stopping.lock() = Some(task);
  }

  fn close(&self, kind: CloseKind, outcome: ShutdownOutcome) {
    let Some(proceed) = self.inner.request.lock().take() else {
      return;
    };
    let end = session_end(kind, outcome);
    tracing::info!(?kind, ?outcome, ?end, "closing the window");
    *self.inner.end.lock() = Some(end);
    proceed();
  }

  /// How the session ended, once the window was let close through this flow; the runners have
  /// stopped then.
  pub fn session_end(&self) -> Option<SessionEnd> {
    *self.inner.end.lock()
  }
}
