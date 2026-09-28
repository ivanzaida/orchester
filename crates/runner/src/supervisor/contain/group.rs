//! The child's own process group (macOS).

use std::io;

use orchester_exec::process_group::{GroupEnd, ProcessGroup, TERMINATE_GRACE};
use tokio::process::Child;

pub(in crate::supervisor) struct Containment {
  /// `None` once the group has been ended, so dropping signals nothing that could reuse its id.
  group: Option<ProcessGroup>,
}

impl Containment {
  pub(in crate::supervisor) fn of(child: &Child) -> io::Result<Self> {
    let id = child
      .id()
      .ok_or_else(|| io::Error::other("the process exited before its group was known"))?;
    let group = ProcessGroup::led_by(id).ok_or_else(|| io::Error::other(format!("invalid process id {id}")))?;
    Ok(Self { group: Some(group) })
  }

  /// Ends the child if it still runs (SIGTERM, SIGKILL after the grace period), reaps it, then
  /// ends whatever it left running in its group the same way.
  pub(in crate::supervisor) async fn end(mut self, child: &mut Child) {
    let Some(group) = self.group.take() else {
      return;
    };
    if matches!(child.try_wait(), Ok(None)) {
      end_leader(group, child).await;
    }
    if let Err(error) = child.wait().await {
      tracing::warn!("cannot confirm that the process ended: {error}");
    }
    match tokio::task::spawn_blocking(move || group.end(TERMINATE_GRACE)).await {
      Ok(Ok(GroupEnd::Exited)) => {}
      Ok(Ok(GroupEnd::Killed)) => {
        tracing::warn!("processes the program left running did not exit on SIGTERM; killed them");
      }
      Ok(Err(error)) => tracing::warn!("cannot end the processes the program left running: {error}"),
      Err(error) => tracing::warn!("ending the processes the program left running failed: {error}"),
    }
  }
}

async fn end_leader(group: ProcessGroup, child: &mut Child) {
  if let Err(error) = group.terminate() {
    tracing::warn!("cannot ask the process to exit: {error}");
  }
  if tokio::time::timeout(TERMINATE_GRACE, child.wait()).await.is_ok() {
    return;
  }
  tracing::warn!("the process did not exit within {TERMINATE_GRACE:?} of SIGTERM; killing it");
  if let Err(error) = group.kill() {
    tracing::warn!("cannot kill the process: {error}");
  }
}

/// The supervising task was dropped (the runner is exiting) before it ended the group: kill it
/// at once, as closing a Job Object does.
impl Drop for Containment {
  fn drop(&mut self) {
    if let Some(group) = self.group.take()
      && let Err(error) = group.kill()
    {
      tracing::warn!("cannot kill the process group: {error}");
    }
  }
}
