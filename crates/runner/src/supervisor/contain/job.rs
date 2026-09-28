//! A kill-on-close Job Object per child (Windows).

use std::io;

use tokio::process::Child;

pub(in crate::supervisor) struct Containment {
  _job: orchester_ipc::KillOnCloseJob,
}

impl Containment {
  pub(in crate::supervisor) fn of(child: &Child) -> io::Result<Self> {
    let job = orchester_ipc::KillOnCloseJob::new()?;
    let handle = child
      .raw_handle()
      .ok_or_else(|| io::Error::other("the process exited before it could join its job"))?;
    job.assign(handle)?;
    Ok(Self { _job: job })
  }

  /// Ends the child if it still runs, with its tree, and reaps it.
  pub(in crate::supervisor) async fn end(self, child: &mut Child) {
    // Closing the job ends the child and anything it left running.
    drop(self);
    if let Err(error) = child.wait().await {
      tracing::warn!("cannot confirm that the process ended: {error}");
    }
  }
}
