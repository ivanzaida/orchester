//! The scheduler's side of the disk guard (ORC-114): no run starts or continues while the disk the workspaces are on
//! has less free space than the project keeps, and runs paused for low disk space go on by themselves once there is
//! room — each in the workspace it used last if that one is free, else the one idle longest.

use std::{
  sync::atomic::{AtomicBool, Ordering},
  time::Duration,
};

use orchester_core::{
  run::{PauseReason, RunStatus},
  store::{RunStore, WorkspaceStore},
  workspaces::{DiskVerdict, GIGABYTE},
};
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;

use super::{Scheduler, failed};

/// How often the watch looks at the free space while runs wait paused for low disk space.
const WATCH_INTERVAL: Duration = Duration::from_secs(30);

/// Whether the watch over runs paused for low disk space runs: one per scheduler.
#[derive(Default)]
pub(super) struct DiskWatch(AtomicBool);

impl Scheduler {
  /// Refused while the disk the workspaces are on has less free space than the project keeps, in a project that
  /// turned warm workspaces on (in any other the guard holds nothing back). A disk that cannot be read holds nothing
  /// back either; the guard logs it.
  pub(super) fn check_disk(&self, db: &ProjectDb) -> Result<(), Rejection> {
    let settings = db
      .workspace_settings()
      .map_err(|error| failed("cannot read the workspace settings", &error))?;
    if !settings.enabled {
      return Ok(());
    }
    let keep = settings.keep_free;
    match self.inner.disks.usage(&self.inner.project_root) {
      Ok(usage) => match keep.verdict(usage) {
        DiskVerdict::Room => Ok(()),
        DiskVerdict::Low { free, needed } => Err(Rejection::LowDisk {
          free_gb: free / GIGABYTE,
          needed_gb: needed / GIGABYTE,
        }),
      },
      Err(error) => {
        tracing::warn!(%error, "the disk guard cannot read the free space");
        Ok(())
      }
    }
  }

  /// Starts the watch that resumes runs paused for low disk space once the disk has room, unless it runs already.
  pub(super) fn watch_disk(&self) {
    if self.inner.disk_watch.0.swap(true, Ordering::AcqRel) {
      return;
    }
    let scheduler = self.clone();
    tokio::spawn(async move {
      loop {
        tokio::time::sleep(WATCH_INTERVAL).await;
        if scheduler.inner.board.is_stopping() {
          break;
        }
        scheduler.resume_after_low_disk().await;
      }
    });
  }

  /// Resumes the runs paused for low disk space, oldest first, while the disk has room and they may start.
  pub(super) async fn resume_after_low_disk(&self) {
    let paused = match self.open().and_then(|db| {
      self.check_disk(&db)?;
      db.low_disk_runs()
        .map_err(|error| failed("cannot read the runs paused for low disk space", &error))
    }) {
      Ok(paused) => paused,
      Err(Rejection::LowDisk { .. }) => return,
      Err(rejection) => {
        tracing::warn!(?rejection, "runs paused for low disk space cannot be read");
        return;
      }
    };
    let low_disk = paused
      .into_iter()
      .filter_map(Result::ok)
      .filter(|run| run.status() == &RunStatus::Paused(PauseReason::LowDisk));
    for run in low_disk {
      match self.resume_run(run.id()).await {
        Ok(()) => tracing::info!(run = %run.id(), "the disk has room again; the run goes on"),
        Err(rejection) => {
          tracing::info!(run = %run.id(), ?rejection, "a run paused for low disk space waits on");
          break;
        }
      }
    }
  }
}
