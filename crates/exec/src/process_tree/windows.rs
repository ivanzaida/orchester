//! The tree as a kill-on-close Job Object (Windows).

use std::{io, os::windows::io::AsRawHandle, process::Child};

use win32job::{ExtendedLimitInfo, Job};

/// The job the program joined; closing it ends every process in it.
pub(super) struct Hold {
  job: Option<Job>,
}

impl Hold {
  /// The tree of `child`, which joins a job of its own right after it started.
  pub(super) fn of(child: &Child) -> io::Result<Self> {
    let job = Job::create_with_limit_info(ExtendedLimitInfo::new().limit_kill_on_job_close())?;
    job.assign_process(child.as_raw_handle() as isize)?;
    Ok(Self { job: Some(job) })
  }

  /// Closes the job, which ends every process of the tree, then reaps `child`.
  pub(super) fn end(&mut self, child: &mut Child) -> io::Result<()> {
    drop(self.job.take());
    child.wait().map(drop)
  }
}
