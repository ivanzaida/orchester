//! The script's tree as a kill-on-close Job Object (Windows).

use std::{io, os::windows::io::AsRawHandle, process::Child};

use orchester_ipc::KillOnCloseJob;

/// Dropping it ends every process of the tree.
pub(in super::super) struct Tree {
  job: Option<KillOnCloseJob>,
}

impl Tree {
  /// The tree of `child`, which joins its job right after it started.
  pub(in super::super) fn of(child: &Child) -> io::Result<Self> {
    let job = KillOnCloseJob::new()?;
    job.assign(child.as_raw_handle())?;
    Ok(Self { job: Some(job) })
  }

  /// Lets what the tree still runs (the QA app) outlive it.
  pub(in super::super) fn keep(mut self) -> io::Result<()> {
    match self.job.take() {
      Some(job) => job.release(),
      None => Ok(()),
    }
  }
}
