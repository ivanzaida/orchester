//! Stopping a runner only once it has no work: the lingering of a runner no tab shows ([`super::lifetime`]) and
//! removing a project from the list ([`Runners::forget_if_idle`]) both ask it. One question is in flight per
//! runner at a time, and every asker gets its answer. Only the runner that was asked is stopped and forgotten,
//! never one started for the project since.

use std::{path::Path, time::Duration};

use orchester_ipc::WorkSummary;
use tokio::sync::watch;

use super::{QUIET_STOP_TIMEOUT, Runners, state::RunnerState};

/// How long removing a project waits for its runner's answer before it gives up, leaving the runner as it is.
const IDLE_ANSWER: Duration = Duration::from_secs(30);

/// Why a project's runner cannot be stopped to forget the project.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForgetRefusal {
  /// Agents are at work in the project, or a request is about to give it work.
  Busy,
  /// The runner is starting, reconnecting or stopping: whether agents are at work is not known.
  Unknown,
  /// The runner did not answer in time; it keeps running.
  NoAnswer,
}

/// Why a runner could not be asked whether it is idle.
pub(super) enum NotAsked {
  /// The project has no runner any more.
  Gone,
  /// A request is about to give it work, or it has no connection to ask through.
  Held,
}

/// The answer to the question in flight: `None` until it came, then whether the runner stopped.
pub(super) type IdleAnswer = watch::Receiver<Option<bool>>;

impl Runners {
  /// Asks project `root`'s runner to stop if it has no work, or joins the question already in flight. The
  /// runner's generation tells which runner was asked.
  pub(super) fn ask_idle(&self, root: &Path) -> Result<(u64, IdleAnswer), NotAsked> {
    let mut projects = self.inner.projects.lock();
    let entry = projects.get_mut(root).ok_or(NotAsked::Gone)?;
    if entry.pins > 0 {
      return Err(NotAsked::Held);
    }
    if let Some(asking) = &entry.asking {
      return Ok((entry.generation, asking.clone()));
    }
    let reply = entry.connection.as_ref().ok_or(NotAsked::Held)?.stop_if_idle();
    let (answer, asking) = watch::channel(None);
    entry.asking = Some(asking.clone());
    let (runners, root, generation) = (self.clone(), root.to_path_buf(), entry.generation);
    self.inner.runtime.spawn(async move {
      answer.send_replace(Some(reply.await == Ok(true)));
      if let Some(entry) = runners.inner.projects.lock().get_mut(&root)
        && entry.generation == generation
      {
        entry.asking = None;
      }
    });
    Ok((generation, asking))
  }

  /// Stops project `root`'s runner of `generation`, which answered that it stopped, waits for its process to
  /// end, and forgets it. Until then its entry stays, so showing the project meanwhile starts no second
  /// runner; then a tab that shows the project starts a new one, unless the project was removed from the list.
  pub(super) async fn retire_idle(&self, root: &Path, generation: u64) {
    let connection = self
      .inner
      .projects
      .lock()
      .get_mut(root)
      .filter(|entry| entry.generation == generation)
      .and_then(|entry| entry.connection.take());
    if let Some(connection) = connection {
      let outcome = connection.stop(Some(QUIET_STOP_TIMEOUT)).await;
      tracing::info!(project = %root.display(), ?outcome, "stopped an idle runner");
    }
    let removed = {
      let mut projects = self.inner.projects.lock();
      match projects.get(root) {
        Some(entry) if entry.generation == generation => projects.remove(root),
        _ => None,
      }
    };
    let Some(entry) = removed else {
      return;
    };
    self.inner.shared.work.send_modify(|revision| *revision += 1);
    if !entry.forgetting && self.tabbed(root) {
      self.ensure(root, &entry.name);
    }
  }

  /// Stops project `root`'s runner only if it has no work, and forgets it once it stopped; nothing it does
  /// is interrupted, and it is not started again for the removed project. A project without a running runner
  /// is forgotten at once. A runner that does not answer in [`IDLE_ANSWER`] keeps running.
  pub async fn forget_if_idle(&self, root: &Path) -> Result<(), ForgetRefusal> {
    if let Some(done) = self.forget_now(root) {
      return done;
    }
    let (generation, mut answer) = match self.ask_idle(root) {
      Ok(asked) => asked,
      Err(NotAsked::Gone) => return Ok(()),
      Err(NotAsked::Held) => return self.refuse(root, ForgetRefusal::Busy),
    };
    let stopped = match tokio::time::timeout(IDLE_ANSWER, answer.wait_for(Option::is_some)).await {
      Ok(Ok(stopped)) => *stopped == Some(true),
      // The question's task ended without an answer: the runner was not stopped.
      Ok(Err(_)) => false,
      Err(_) => return self.refuse(root, ForgetRefusal::NoAnswer),
    };
    if !stopped {
      // A runner that lingered may have been forgotten meanwhile; one that is still there has work.
      let present = self.inner.projects.lock().contains_key(root);
      return if present {
        self.refuse(root, ForgetRefusal::Busy)
      } else {
        Ok(())
      };
    }
    self.retire_idle(root, generation).await;
    Ok(())
  }

  /// What forgetting `root` comes to before asking its runner: done for a runner that is not running,
  /// refused while its work is unknown or agents work; `None` when it is to be asked, marked as being
  /// forgotten so it is not started again.
  fn forget_now(&self, root: &Path) -> Option<Result<(), ForgetRefusal>> {
    let mut projects = self.inner.projects.lock();
    let entry = projects.get_mut(root)?;
    let status = entry.status.borrow().clone();
    match (status.state(), status.work().map(WorkSummary::agents_working)) {
      (RunnerState::Stopped | RunnerState::Failed { .. }, _) => {
        projects.remove(root);
        Some(Ok(()))
      }
      (RunnerState::Online, Some(0)) if entry.pins == 0 => {
        entry.forgetting = true;
        None
      }
      (RunnerState::Online, Some(_)) => Some(Err(ForgetRefusal::Busy)),
      // Starting, reconnecting or stopping, or online before its first report: its work is not known.
      _ => Some(Err(ForgetRefusal::Unknown)),
    }
  }

  /// `refusal`, with project `root`'s runner no longer marked as being forgotten.
  fn refuse(&self, root: &Path, refusal: ForgetRefusal) -> Result<(), ForgetRefusal> {
    if let Some(entry) = self.inner.projects.lock().get_mut(root) {
      entry.forgetting = false;
    }
    Err(refusal)
  }
}
