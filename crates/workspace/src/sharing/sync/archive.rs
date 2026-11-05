//! A team resource archived on the server stops the runs here that pin it: each run that has not ended and pins any
//! version of it — its pipeline, a step's profile or tool, an instruction, tool or skill — is cancelled through its
//! project's runner, and recorded ([`ArchiveStop`]) so the person is told which run stopped and why
//! (`crate::notifications::sources::of_archive_stops`). A run that cannot be cancelled is logged and not recorded: no
//! notification says it stopped.

use std::path::Path;

use orchester_core::{
  id::{ResourceId, RunId, TaskId},
  name::Name,
  store::ResourceStore,
  time::Timestamp,
};
use orchester_ipc::Command;
use orchester_store::{ArchiveStop, ProjectDb, RunStoreError, StoreError};

use crate::{background, runner::Runners, tasks::run::control_message, workspace::OpenProject};

/// Why the runs an archived resource stops could not be read or recorded.
#[derive(Debug, thiserror::Error)]
enum ArchiveError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error(transparent)]
  Runs(#[from] RunStoreError),
}

/// The runs of the project at `root` that `resource` stops, with the project and the resource's name.
struct Pinning {
  project: OpenProject,
  resource: Name,
  runs: Vec<(RunId, TaskId)>,
}

/// Cancels every run of the project at `root` that pins `resource`, archived on the server, and records each one
/// cancelled.
pub(super) async fn stop_runs(runners: &Runners, root: &Path, resource: ResourceId) {
  let owned = root.to_owned();
  let pinning = match background::run(move || pinning(&owned, resource)).await {
    Ok(Ok(Some(pinning))) => pinning,
    Ok(Ok(None)) => return,
    Ok(Err(error)) => {
      tracing::error!(%error, %resource, "cannot read the runs an archived resource stops");
      return;
    }
    // The blocking pool is shutting down with the window.
    Err(_) => return,
  };
  for (run, task) in pinning.runs {
    if let Err(error) = runners
      .control_run_in(&pinning.project, Command::CancelRun { run })
      .await
    {
      let message = control_message(&error);
      tracing::warn!(?message, %run, %resource, "cannot stop a run that pins an archived resource");
      continue;
    }
    let stop = ArchiveStop {
      run,
      task,
      resource: pinning.resource.clone(),
      stopped_at: Timestamp::now(),
    };
    let owned = root.to_owned();
    match background::run(move || record(&owned, &stop)).await {
      Ok(Ok(())) => {}
      Ok(Err(error)) => tracing::error!(%error, %run, "cannot record a run stopped by an archived resource"),
      // The blocking pool is shutting down with the window.
      Err(_) => return,
    }
  }
}

fn record(root: &Path, stop: &ArchiveStop) -> Result<(), StoreError> {
  ProjectDb::open(root)?.record_archive_stop(stop)
}

/// The runs that have not ended and pin `resource`; `None` when the project does not keep it.
fn pinning(root: &Path, resource: ResourceId) -> Result<Option<Pinning>, ArchiveError> {
  let db = ProjectDb::open(root)?;
  let Some(kept) = db.resource(resource)? else {
    return Ok(None);
  };
  let runs = db
    .unfinished_runs()?
    .into_iter()
    .filter_map(|listed| match listed {
      Ok(run) => run.snapshot().pins(resource).then(|| (run.id(), run.task())),
      Err(unreadable) => {
        tracing::warn!(
          ?unreadable,
          "an unreadable run is not checked against an archived resource"
        );
        None
      }
    })
    .collect();
  Ok(Some(Pinning {
    project: OpenProject {
      project: db.project().clone(),
      repo_root: root.to_owned(),
    },
    resource: kept.name().clone(),
    runs,
  }))
}
