//! Closing the window closes the application (`docs/architecture/storage.md`, "Lifecycle").
//! Without active work the runner is stopped quietly; with active work the close is held for
//! a confirmation: wait for agents to save, quit now, or cancel.

use std::sync::Arc;

use lurq::{app::CloseRequest, core::Signal};
use orchester_store::SessionEnd;
use parking_lot::Mutex;
use tokio::{runtime::Handle, task::JoinHandle};

use crate::runner::{ActiveWork, QUIET_STOP_TIMEOUT, Runners, ShutdownOutcome};

/// What a close request leads to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClosePlan {
  /// Nothing would be interrupted: stop the runner and close.
  StopQuietly,
  /// Ask first, listing the work in progress.
  Confirm(Arc<[ActiveWork]>),
}

pub fn close_plan(active: Vec<ActiveWork>) -> ClosePlan {
  if active.is_empty() {
    ClosePlan::StopQuietly
  } else {
    ClosePlan::Confirm(active.into())
  }
}

/// How the application was closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseKind {
  /// No work was active.
  Quiet,
  /// The user chose "Wait for agents to save".
  WaitForAgents,
  /// The user chose "Quit now".
  QuitNow,
}

/// How the session ends, given how it was closed and how the runner stopped. A runner that
/// did not confirm its stop was terminated, which is a quit-now end.
pub fn session_end(kind: CloseKind, outcome: ShutdownOutcome) -> SessionEnd {
  match (kind, outcome) {
    (CloseKind::QuitNow, _) | (_, ShutdownOutcome::Unanswered) => SessionEnd::QuitNow,
    (CloseKind::WaitForAgents, ShutdownOutcome::Stopped) => SessionEnd::RunsPaused,
    (CloseKind::Quiet, _) | (CloseKind::WaitForAgents, ShutdownOutcome::NoRunner) => SessionEnd::Clean,
  }
}

/// Where the close confirmation stands; set on the UI thread only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClosePhase {
  Open,
  /// The close is held while the user decides.
  Confirming(Arc<[ActiveWork]>),
  /// "Wait for agents to save" was chosen; the runner is stopping at checkpoints.
  Waiting(Arc<[ActiveWork]>),
  /// Quiet close in progress.
  Closing,
}

impl lurq::app::component::DevtoolsInspectable for ClosePhase {}

/// The application's close flow, shared by the window's close handler, the confirmation and
/// `main`, which records how the session ended.
#[derive(Clone)]
pub struct Lifecycle {
  inner: Arc<Inner>,
}

struct Inner {
  runners: Runners,
  runtime: Handle,
  phase: Signal<ClosePhase>,
  /// The held close request; whoever takes it decides how the session ends.
  request: Mutex<Option<CloseRequest>>,
  end: Mutex<Option<SessionEnd>>,
  stopping: Mutex<Option<JoinHandle<()>>>,
}

impl PartialEq for Lifecycle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.inner, &other.inner)
  }
}

impl lurq::app::component::DevtoolsInspectable for Lifecycle {}

impl Lifecycle {
  pub fn new(runners: Runners, runtime: Handle) -> Self {
    Self {
      inner: Arc::new(Inner {
        runners,
        runtime,
        phase: Signal::new(ClosePhase::Open),
        request: Mutex::new(None),
        end: Mutex::new(None),
        stopping: Mutex::new(None),
      }),
    }
  }

  pub fn runners(&self) -> &Runners {
    &self.inner.runners
  }

  pub fn phase(&self) -> Signal<ClosePhase> {
    self.inner.phase.clone()
  }

  /// The window's close handler. A request while a close is already under way is dropped,
  /// which keeps the window open until that close finishes.
  pub fn on_close_requested(&self, request: CloseRequest) {
    if self.inner.phase.get_untracked() != ClosePhase::Open {
      return;
    }
    *self.inner.request.lock() = Some(request);
    match close_plan(self.inner.runners.active_work()) {
      ClosePlan::StopQuietly => {
        self.inner.phase.set(ClosePhase::Closing);
        self.stop_then_close(CloseKind::Quiet, Some(QUIET_STOP_TIMEOUT));
      }
      ClosePlan::Confirm(active) => self.inner.phase.set(ClosePhase::Confirming(active)),
    }
  }

  pub fn wait_for_agents(&self) {
    let ClosePhase::Confirming(active) = self.inner.phase.get_untracked() else {
      return;
    };
    self.inner.phase.set(ClosePhase::Waiting(active));
    // No deadline: the confirmation keeps offering Quit now while agents save.
    self.stop_then_close(CloseKind::WaitForAgents, None);
  }

  pub fn quit_now(&self) {
    if let Some(stopping) = self.inner.stopping.lock().take() {
      stopping.abort();
    }
    self.inner.runners.terminate();
    self.close(CloseKind::QuitNow, ShutdownOutcome::Unanswered);
  }

  /// Keeps the application open.
  pub fn cancel(&self) {
    if !matches!(self.inner.phase.get_untracked(), ClosePhase::Confirming(_)) {
      return;
    }
    drop(self.inner.request.lock().take());
    self.inner.phase.set(ClosePhase::Open);
  }

  fn stop_then_close(&self, kind: CloseKind, deadline: Option<std::time::Duration>) {
    let lifecycle = self.clone();
    let task = self.inner.runtime.spawn(async move {
      let outcome = lifecycle.inner.runners.shutdown(deadline).await;
      lifecycle.close(kind, outcome);
    });
    *self.inner.stopping.lock() = Some(task);
  }

  fn close(&self, kind: CloseKind, outcome: ShutdownOutcome) {
    let Some(request) = self.inner.request.lock().take() else {
      return;
    };
    let end = session_end(kind, outcome);
    tracing::info!(?kind, ?outcome, ?end, "closing the application");
    *self.inner.end.lock() = Some(end);
    request.proceed();
  }

  /// How the session ended, once the window closed through this flow.
  pub fn session_end(&self) -> Option<SessionEnd> {
    *self.inner.end.lock()
  }
}
