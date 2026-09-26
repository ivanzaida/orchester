//! Keeping a child's process tree together so it can be ended as a whole.
//!
//! - Windows: a kill-on-close Job Object per child. Closing it ends the child and every process it started, and nothing
//!   survives the runner.
//! - macOS: not supported yet (a known gap in `AGENTS.md`, "process-tree kill"): only the child itself is ended;
//!   processes it started and left running are not.

use std::io;

use tokio::process::Child;

pub(super) struct Containment {
  #[cfg(windows)]
  _job: orchester_ipc::KillOnCloseJob,
}

impl Containment {
  #[cfg(windows)]
  pub(super) fn of(child: &Child) -> io::Result<Self> {
    let job = orchester_ipc::KillOnCloseJob::new()?;
    let handle = child
      .raw_handle()
      .ok_or_else(|| io::Error::other("the process exited before it could join its job"))?;
    job.assign(handle)?;
    Ok(Self { _job: job })
  }

  #[cfg(not(windows))]
  #[expect(clippy::unnecessary_wraps, reason = "matches the Windows signature, which can fail")]
  pub(super) fn of(_child: &Child) -> io::Result<Self> {
    tracing::warn!("processes started by an agent CLI are not ended with it on this platform yet");
    Ok(Self {})
  }

  /// Ends the child if it still runs, with its tree where supported, and reaps it.
  pub(super) async fn end(self, child: &mut Child) {
    #[cfg(not(windows))]
    if let Err(error) = child.start_kill() {
      tracing::debug!(%error, "the process had already exited");
    }
    // Closing the job ends the child and anything it left running.
    drop(self);
    if let Err(error) = child.wait().await {
      tracing::warn!("cannot confirm that the process ended: {error}");
    }
  }
}
