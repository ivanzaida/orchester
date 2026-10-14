use std::{io, os::windows::io::RawHandle};

use win32job::{ExtendedLimitInfo, Job};

/// A Windows Job Object that terminates every process in it when its last handle closes, so
/// the processes it holds cannot outlive the owner, even if the owner crashes.
///
/// Processes started by an assigned process join the job too. A process can be in several
/// nested jobs, so the runner's per-child jobs live inside the desktop's job for the runner.
#[derive(Debug)]
pub struct KillOnCloseJob {
  job: Job,
}

impl KillOnCloseJob {
  pub fn new() -> io::Result<Self> {
    let job = Job::create_with_limit_info(ExtendedLimitInfo::new().limit_kill_on_job_close())?;
    Ok(Self { job })
  }

  /// Adds a running process, given its process handle, to the job.
  pub fn assign(&self, process: RawHandle) -> io::Result<()> {
    self.job.assign_process(process as isize)?;
    Ok(())
  }

  /// Closes the job without ending what runs in it: kill-on-close is cleared first, so the processes left in it (an
  /// app a script started to run on) outlive it.
  pub fn release(self) -> io::Result<()> {
    self.job.set_extended_limit_info(&ExtendedLimitInfo::new())?;
    Ok(())
  }
}
