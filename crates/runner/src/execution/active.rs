//! A run this runner executes: the way to steer its task, the task itself and whether it settled.

use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

use tokio::{sync::mpsc, task::JoinHandle};

use super::control::Control;

pub(super) struct ActiveRun {
  pub(super) control: mpsc::UnboundedSender<Control>,
  pub(super) task: JoinHandle<()>,
  /// Set by the task once it no longer executes the run, before the change that shows it can be
  /// read: the run settled (paused, stopped at a limit, cancelled, interrupted or ended), waits
  /// for a person with nothing else to do, or the task failed. The task only ends after.
  pub(super) settled: Arc<AtomicBool>,
}

impl ActiveRun {
  /// Whether the task has ended. It lets go of its requests as it drops its state, once its last
  /// change is recorded and its agents have ended; its [`JoinHandle`] reports it finished only a
  /// moment later.
  pub(super) fn ended(&self) -> bool {
    self.control.is_closed()
  }

  /// Whether the task still executes the run: it counts among the active runs.
  pub(super) fn executes(&self) -> bool {
    !self.ended() && !self.settled.load(Ordering::Acquire)
  }
}
