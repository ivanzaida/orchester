//! The scheduler's side of the disk guard (ORC-114, ORC-422): in every project, no run starts or continues while the
//! repository's disk has less free space than the project keeps, and runs paused for low disk space go on by
//! themselves once there is room — a borrowing run in the workspace it used last if that one is free, else the one
//! idle longest.

use std::{
  path::Path,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  time::Duration,
};

use orchester_core::{
  run::{PauseReason, RunStatus},
  store::{RunStore, SettingsStore},
  workspaces::{DiskSpace, DiskSpaceError, DiskUsage, DiskVerdict, GIGABYTE},
};
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;

use super::{Scheduler, failed};

/// How often the disk guard looks at the free space while agents work, and while runs wait paused for low disk space.
const CHECK_INTERVAL: Duration = Duration::from_secs(30);

/// The disk guard's reading of free space, and how often it looks.
#[derive(Clone)]
pub(super) struct DiskGuard {
  space: Arc<dyn DiskSpace>,
  pub(super) every: Duration,
}

impl DiskGuard {
  /// The guard over this computer's disks.
  pub(super) fn local() -> Self {
    Self {
      space: Arc::new(orchester_exec::disk_space::LocalDisks),
      every: CHECK_INTERVAL,
    }
  }

  pub(super) fn usage(&self, folder: &Path) -> Result<DiskUsage, DiskSpaceError> {
    self.space.usage(folder)
  }
}

/// Whether the watch over runs paused for low disk space runs: one per scheduler, and only while such runs wait.
#[derive(Default)]
pub(super) struct DiskWatch(AtomicBool);

impl Scheduler {
  /// This scheduler reading free space through `disks`, before it is shared: what tests give the disk guard.
  #[must_use]
  pub fn with_disks(mut self, disks: Arc<dyn DiskSpace>) -> Self {
    if let Some(inner) = Arc::get_mut(&mut self.inner) {
      inner.disks.space = disks;
    }
    self
  }

  /// This scheduler's disk guard looking at the free space `every` so often, before it is shared: what tests give it
  /// instead of every 30 seconds.
  #[must_use]
  pub fn with_disk_checks(mut self, every: Duration) -> Self {
    if let Some(inner) = Arc::get_mut(&mut self.inner) {
      inner.disks.every = every;
    }
    self
  }

  /// Refused while the repository's disk, where every run's folder is, has less free space than the project keeps,
  /// whether its runs borrow warm workspaces or own a worktree. What it keeps is read at every check, so a change
  /// applies without a restart. A disk that cannot be read holds nothing back; the guard logs it.
  pub(super) fn check_disk(&self, db: &ProjectDb) -> Result<(), Rejection> {
    let keep = db
      .keep_free()
      .map_err(|error| failed("cannot read the free space the project keeps", &error))?;
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

  /// Starts the watch that resumes runs paused for low disk space once the disk has room, unless it runs already. It
  /// ends once no run waits paused for low disk space; a run that pauses later starts it again.
  pub(super) fn watch_disk(&self) {
    if self.inner.disk_watch.0.swap(true, Ordering::AcqRel) {
      return;
    }
    let scheduler = self.clone();
    tokio::spawn(async move {
      loop {
        tokio::time::sleep(scheduler.inner.disks.every).await;
        if scheduler.inner.board.is_stopping() || !scheduler.resume_after_low_disk().await {
          break;
        }
      }
      scheduler.inner.disk_watch.0.store(false, Ordering::Release);
      // A run that paused while the watch was ending found it running: it is watched again.
      if !scheduler.inner.board.is_stopping() && scheduler.waits_for_disk() {
        scheduler.watch_disk();
      }
    });
  }

  /// Resumes the runs paused for low disk space, oldest first, while the disk has room and they may start. `true`
  /// while any of them still waits, or they cannot be read.
  pub(super) async fn resume_after_low_disk(&self) -> bool {
    let paused = match self.open().and_then(|db| {
      self.check_disk(&db)?;
      db.low_disk_runs()
        .map_err(|error| failed("cannot read the runs paused for low disk space", &error))
    }) {
      Ok(paused) => paused,
      Err(Rejection::LowDisk { .. }) => return true,
      Err(rejection) => {
        tracing::warn!(?rejection, "runs paused for low disk space cannot be read");
        return true;
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
          return true;
        }
      }
    }
    false
  }

  /// Whether a run waits paused for low disk space; a database that cannot be read counts as one.
  fn waits_for_disk(&self) -> bool {
    self.open().map_or(true, |db| {
      db.low_disk_runs().map_or(true, |runs| {
        runs
          .into_iter()
          .filter_map(Result::ok)
          .any(|run| run.status() == &RunStatus::Paused(PauseReason::LowDisk))
      })
    })
  }
}
