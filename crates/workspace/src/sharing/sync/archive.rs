//! A team resource archived on the server stops the runs here that pin it: each run that has not ended and pins any
//! version of it — its pipeline, a step's profile or tool, an instruction, tool or skill — or, for a team MCP server,
//! pins a profile version that chooses it, is cancelled through its project's runner, and recorded ([`ArchiveStop`]) so
//! the person is told which run stopped and why (`crate::notifications::sources::of_archive_stops`). A run that cannot
//! be cancelled is logged and not recorded: no notification says it stopped.

use std::path::Path;

use orchester_core::{
  id::{ConnectionId, ResourceId, RunId, TaskId},
  name::Name,
  resource::{AgentProfile, Resource, ResourceKind},
  run::{RunSnapshot, StepExecutor},
  store::{McpServerStore, ResourceStore},
  time::Timestamp,
};
use orchester_ipc::Command;
use orchester_store::{ArchiveStop, Archived, ProjectDb, ReferenceTarget, RunStoreError, StoreError};

use crate::{background, runner::Runners, tasks::run::control_message, workspace::OpenProject};

/// Why the runs an archived resource stops could not be read or recorded.
#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error(transparent)]
  Runs(#[from] RunStoreError),
}

/// The runs of the project at `root` that `resource` stops, with the project and the resource's name.
pub struct Pinning {
  project: OpenProject,
  resource: Name,
  kind: ResourceKind,
  runs: Vec<(RunId, TaskId)>,
}

impl Pinning {
  /// What was archived: a resource's kind, or an MCP server.
  pub const fn kind(&self) -> ResourceKind {
    self.kind
  }

  /// The archived resource's name, as the *Stopped* notification names it.
  pub fn resource(&self) -> &Name {
    &self.resource
  }

  /// Each run that stops, with its task.
  pub fn runs(&self) -> &[(RunId, TaskId)] {
    &self.runs
  }
}

/// Cancels every run of the project at `root` that pins `resource`, archived on the server by `by` (when the server
/// said), and records each one cancelled.
pub(super) async fn stop_runs(runners: &Runners, root: &Path, (resource, by): (ResourceId, Option<&str>)) {
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
  let archived = Archived {
    id: resource,
    kind: pinning.kind,
    by: by.map(str::to_owned),
  };
  stop(runners, root, pinning, (Some(archived), None)).await;
}

/// What stopping the runs of an archive did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct StoppedRuns {
  /// The runs the runner cancelled.
  pub runs: Vec<RunId>,
  /// How many it could not: they keep working.
  pub left_running: usize,
}

/// Cancels the runs `pinning` names through the project's runner and records each one cancelled, with what was
/// archived when it is a resource, or the local MCP server `server` whose archive it was.
pub(crate) async fn stop(
  runners: &Runners,
  root: &Path,
  pinning: Pinning,
  (archived, server): (Option<Archived>, Option<ConnectionId>),
) -> StoppedRuns {
  let mut stopped = StoppedRuns::default();
  for (run, task) in pinning.runs {
    if let Err(error) = runners
      .control_run_in(&pinning.project, Command::CancelRun { run })
      .await
    {
      let message = control_message(&error);
      tracing::warn!(?message, %run, resource = %pinning.resource, "cannot stop a run that pins an archived resource");
      stopped.left_running += 1;
      continue;
    }
    let stop = ArchiveStop {
      run,
      task,
      resource: pinning.resource.clone(),
      stopped_at: Timestamp::now(),
      archived: archived.clone(),
    };
    stopped.runs.push(run);
    let owned = root.to_owned();
    match background::run(move || record(&owned, &stop, server)).await {
      Ok(Ok(())) => {}
      Ok(Err(error)) => tracing::error!(%error, %run, "cannot record a run stopped by an archived resource"),
      // The blocking pool is shutting down with the window.
      Err(_) => return stopped,
    }
  }
  stopped
}

fn record(root: &Path, stop: &ArchiveStop, server: Option<ConnectionId>) -> Result<(), StoreError> {
  let mut db = ProjectDb::open(root)?;
  db.record_archive_stop(stop)?;
  server.map_or(Ok(()), |server| db.record_mcp_server_stop(stop.run, server))
}

/// The runs that have not ended and pin `resource`; `None` when the project does not keep it.
pub fn pinning(root: &Path, resource: ResourceId) -> Result<Option<Pinning>, ArchiveError> {
  pinning_in(&ProjectDb::open(root)?, root, resource)
}

/// [`pinning`] read through `db`, which an archive holds in the transaction that writes it.
pub(crate) fn pinning_in(db: &ProjectDb, root: &Path, resource: ResourceId) -> Result<Option<Pinning>, ArchiveError> {
  let Some(kept) = db.resource(resource)? else {
    return Ok(None);
  };
  let mut runs = Vec::new();
  for listed in db.unfinished_runs()? {
    match listed {
      Ok(run) => {
        if pins(db, run.snapshot(), &kept)? {
          runs.push((run.id(), run.task()));
        }
      }
      Err(unreadable) => tracing::warn!(
        ?unreadable,
        "an unreadable run is not checked against an archived resource"
      ),
    }
  }
  Ok(Some(Pinning {
    project: OpenProject {
      project: db.project().clone(),
      repo_root: root.to_owned(),
    },
    resource: kept.name().clone(),
    kind: kept.kind(),
    runs,
  }))
}

/// The runs that have not ended and pin the local MCP server `server`; `None` when the project does not keep it.
pub fn pinning_mcp_server(root: &Path, server: ConnectionId) -> Result<Option<Pinning>, ArchiveError> {
  pinning_mcp_server_in(&ProjectDb::open(root)?, root, server)
}

/// [`pinning_mcp_server`] read through `db`, which an archive holds in the transaction that writes it.
pub(crate) fn pinning_mcp_server_in(
  db: &ProjectDb,
  root: &Path,
  server: ConnectionId,
) -> Result<Option<Pinning>, ArchiveError> {
  let Some(kept) = db.mcp_server(server)? else {
    return Ok(None);
  };
  let mut runs = Vec::new();
  for listed in db.unfinished_runs()? {
    match listed {
      Ok(run) => {
        if pins_mcp_server(db, (run.id(), run.snapshot()), server)? {
          runs.push((run.id(), run.task()));
        }
      }
      Err(unreadable) => tracing::warn!(
        ?unreadable,
        "an unreadable run is not checked against an archived MCP server"
      ),
    }
  }
  Ok(Some(Pinning {
    project: OpenProject {
      project: db.project().clone(),
      repo_root: root.to_owned(),
    },
    resource: Name::new(kept.name().as_str()).expect("an MCP server's name is a valid name"),
    kind: ResourceKind::McpServer,
    runs,
  }))
}

/// Whether `run`, with `snapshot`, uses the local MCP server `server`: a local server has no versions to pin, so a run
/// uses the servers its pinned profile versions choose and the ones its recorded inputs bind by id.
pub(crate) fn pins_mcp_server(
  db: &ProjectDb,
  (run, snapshot): (RunId, &RunSnapshot),
  server: ConnectionId,
) -> Result<bool, StoreError> {
  if db
    .run_inputs(run)?
    .is_some_and(|inputs| inputs.bindings().binds_local_mcp(server))
  {
    return Ok(true);
  }
  for step in snapshot.steps() {
    let StepExecutor::Agent { profile, .. } = step.executor() else {
      continue;
    };
    let version = db.version::<AgentProfile>(profile.resource, profile.number)?;
    if version.is_some_and(|version| version.body().mcp_servers().ids().contains(&server)) {
      return Ok(true);
    }
  }
  Ok(false)
}

/// Whether a run with `snapshot` pins `resource`: the one rule for the runs an archive stops, also read by the used-by
/// list before the archive (`crate::used_by`), so for a local project the list is what the archive stops. A run pins no
/// MCP server of its own: it uses the servers its pinned profile versions choose, and a team profile version chooses a
/// team MCP server by a `TeamResource { kind: McpServer, id }` reference, read from the cached version.
pub(crate) fn pins(db: &ProjectDb, snapshot: &RunSnapshot, resource: &Resource) -> Result<bool, StoreError> {
  if snapshot.pins(resource.id()) {
    return Ok(true);
  }
  if resource.kind() != ResourceKind::McpServer {
    return Ok(false);
  }
  for step in snapshot.steps() {
    let StepExecutor::Agent { profile, .. } = step.executor() else {
      continue;
    };
    // A profile made here has no cached version and so chooses no team server.
    let Some(cached) = db.cached_version::<AgentProfile>(profile.resource, profile.number)? else {
      continue;
    };
    let chooses = cached.references.iter().any(|reference| {
      matches!(
        reference,
        ReferenceTarget::TeamResource { kind: ResourceKind::McpServer, id } if *id == resource.id()
      )
    });
    if chooses {
      return Ok(true);
    }
  }
  Ok(false)
}
