//! A local project's answer, computed from its database as the team server computes its own: the other resources that
//! are not removed and whose latest published version names the resource ([`super::names`]), by name compared
//! case-insensitively and then by id; and the runs that have not ended and pin any version of it, by the task key's
//! number and then the run id. A local run has no holder: no one else's claim is on its task. Local tasks have no works
//! or claims, so the team server's rule of listing only each task's current work does not apply here. A local project
//! holds no cached team versions today, so the references branch of the resources list only mirrors the team server's
//! rule.

use std::{num::NonZeroU32, path::Path};

use orchester_api::SharedResourceKind;
use orchester_core::{
  id::{ConnectionId, ResourceId, RunId},
  resource::{AgentProfile, Resource, ResourceBody, ResourceKind, TeamMcpServer, TeamReviewer, VersionNumber},
  run::{RunSnapshot, RunStatus},
  store::{McpServerStore, ResourceStore, RunStore, TaskStore},
};
use orchester_store::{ProjectDb, StoreError};

use super::{RunLine, RunState, UsedBy, UsedByError, UsedByResource, UsedByRun, names};
use crate::sharing::sync::{pins, pins_mcp_server};

pub(super) fn used_by(root: &Path, resource: ResourceId) -> Result<UsedBy, UsedByError> {
  let db = ProjectDb::open(root)?;
  // A removed resource answers like an archived team one, so what still uses it can be checked.
  let kept = db.resource(resource)?.ok_or(UsedByError::NotFound)?;
  Ok(UsedBy {
    resources: resources(&db, resource)?,
    runs: runs(&db, &kept)?,
  })
}

/// A local MCP server's answer: the profiles whose latest version chooses it, and the runs that use it (a run uses the
/// servers its pinned profile versions choose). A local MCP server is a connection, not a versioned resource, so it has
/// its own typed id.
pub(super) fn used_by_mcp_server(root: &Path, server: ConnectionId) -> Result<UsedBy, UsedByError> {
  let db = ProjectDb::open(root)?;
  db.mcp_server(server)?.ok_or(UsedByError::NotFound)?;
  let mut profiles = Vec::new();
  for resource in db.resources(ResourceKind::AgentProfile)? {
    let Some(latest) = db.latest_version::<AgentProfile>(resource.id())? else {
      continue;
    };
    if latest.body().mcp_servers().ids().contains(&server) {
      profiles.push(UsedByResource {
        kind: SharedResourceKind::AgentProfile,
        id: resource.id(),
        name: resource.name().to_string(),
        version: latest.number(),
      });
    }
  }
  profiles.sort_by_cached_key(|used| (used.name.to_lowercase(), used.id.to_string()));
  Ok(UsedBy {
    resources: profiles,
    runs: runs_where(&db, |snapshot| pins_mcp_server(&db, snapshot, server))?,
  })
}

/// How each of `runs` stands, for the confirmation that lists the runs an archive stops (`Agents · Archive · used by`
/// L11); a run the project no longer has is left out.
pub(super) fn run_lines(root: &Path, runs: &[RunId]) -> Result<Vec<RunLine>, UsedByError> {
  let db = ProjectDb::open(root)?;
  let mut lines = Vec::new();
  for id in runs {
    let Some(run) = db.run(*id)? else {
      continue;
    };
    lines.push(RunLine {
      run: *id,
      number: run.number().get(),
      state: match run.status() {
        RunStatus::Pending => RunState::Pending,
        RunStatus::Running => RunState::Running,
        RunStatus::Paused(_) => RunState::Paused,
        RunStatus::WaitingForPerson(_) => RunState::Waiting,
        RunStatus::Interrupted(_) => RunState::Interrupted,
        RunStatus::Succeeded | RunStatus::Failed(_) | RunStatus::Cancelled => RunState::Ended,
      },
    });
  }
  Ok(lines)
}

fn resources(db: &ProjectDb, target: ResourceId) -> Result<Vec<UsedByResource>, UsedByError> {
  let mut found = Vec::new();
  naming(db, target, (SharedResourceKind::Pipeline, names::pipeline), &mut found)?;
  naming(
    db,
    target,
    (SharedResourceKind::Instruction, names::instruction),
    &mut found,
  )?;
  naming(db, target, (SharedResourceKind::Skill, names::skill), &mut found)?;
  naming(db, target, (SharedResourceKind::Tool, names::tool), &mut found)?;
  naming(
    db,
    target,
    (SharedResourceKind::AgentProfile, names::never::<AgentProfile>),
    &mut found,
  )?;
  naming(
    db,
    target,
    (SharedResourceKind::McpServer, names::never::<TeamMcpServer>),
    &mut found,
  )?;
  naming(
    db,
    target,
    (SharedResourceKind::TeamReviewer, names::never::<TeamReviewer>),
    &mut found,
  )?;
  found.sort_by_cached_key(|used| (used.name.to_lowercase(), used.id.to_string()));
  Ok(found)
}

/// Adds the resources of `kind`, other than `target`, whose latest version `names` `target` in its body or, when it is
/// a cached team version, in its references.
fn naming<B: ResourceBody>(
  db: &ProjectDb,
  target: ResourceId,
  (kind, names): (SharedResourceKind, fn(&B, ResourceId) -> bool),
  found: &mut Vec<UsedByResource>,
) -> Result<(), UsedByError> {
  for resource in db.resources(kind.core())? {
    if resource.id() == target {
      continue;
    }
    let Some(latest) = db.latest_version::<B>(resource.id())? else {
      continue;
    };
    if names(latest.body(), target) || referenced::<B>(db, resource.id(), latest.number(), target)? {
      found.push(UsedByResource {
        kind,
        id: resource.id(),
        name: resource.name().to_string(),
        version: latest.number(),
      });
    }
  }
  Ok(())
}

/// Whether the cached team version `number` of `resource` has a reference to `target`; a version made here has none.
fn referenced<B: ResourceBody>(
  db: &ProjectDb,
  resource: ResourceId,
  number: VersionNumber,
  target: ResourceId,
) -> Result<bool, UsedByError> {
  Ok(
    db.cached_version::<B>(resource, number)?
      .is_some_and(|cached| names::referenced(&cached.references, target)),
  )
}

/// The runs that have not ended and pin `target` by the rule an archive stops runs by ([`pins`]). A run that cannot
/// be read is logged and left out, as it is when the archive stops runs (`crate::sharing::sync`).
fn runs(db: &ProjectDb, target: &Resource) -> Result<Vec<UsedByRun>, UsedByError> {
  runs_where(db, |snapshot| pins(db, snapshot, target))
}

/// The runs that have not ended and whose snapshot `pinned` says uses the target, by the task key's number and then
/// the run id.
fn runs_where(
  db: &ProjectDb,
  pinned: impl Fn(&RunSnapshot) -> Result<bool, StoreError>,
) -> Result<Vec<UsedByRun>, UsedByError> {
  let mut found: Vec<(NonZeroU32, UsedByRun)> = Vec::new();
  for listed in db.unfinished_runs()? {
    let run = match listed {
      Ok(run) if pinned(run.snapshot())? => run,
      Ok(_) => continue,
      Err(unreadable) => {
        tracing::warn!(
          ?unreadable,
          "an unreadable run is not checked against a resource's users"
        );
        continue;
      }
    };
    let task = db.task(run.task())?.ok_or(UsedByError::TaskMissing(run.task()))?;
    let used = UsedByRun {
      task_key: task.key().to_string(),
      run: run.id(),
      holder: None,
    };
    found.push((task.key().number(), used));
  }
  found.sort_by_key(|(task, used)| (*task, used.run));
  Ok(found.into_iter().map(|(_, used)| used).collect())
}
