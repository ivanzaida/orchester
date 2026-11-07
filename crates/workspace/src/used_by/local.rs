//! A local project's answer, computed from its database as the team server computes its own: the other resources that
//! are not removed and whose latest published version names the resource ([`super::names`]), by name compared
//! case-insensitively and then by id; and the runs that have not ended and pin any version of it, by the task key's
//! number and then the run id. A local run has no holder: no one else's claim is on its task. Local tasks have no works
//! or claims, so the team server's rule of listing only each task's current work does not apply here.

use std::{num::NonZeroU32, path::Path};

use orchester_api::SharedResourceKind;
use orchester_core::{
  id::ResourceId,
  resource::{Resource, ResourceBody},
  store::{ResourceStore, TaskStore},
};
use orchester_store::ProjectDb;

use super::{UsedBy, UsedByError, UsedByResource, UsedByRun, names};
use crate::sharing::sync::pins;

pub(super) fn used_by(root: &Path, resource: ResourceId) -> Result<UsedBy, UsedByError> {
  let db = ProjectDb::open(root)?;
  // A removed resource answers like an archived team one, so what still uses it can be checked.
  let kept = db.resource(resource)?.ok_or(UsedByError::NotFound)?;
  Ok(UsedBy {
    resources: resources(&db, resource)?,
    runs: runs(&db, &kept)?,
  })
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
  found.sort_by_cached_key(|used| (used.name.to_lowercase(), used.id.to_string()));
  Ok(found)
}

/// Adds the resources of `kind`, other than `target`, whose latest version `names` `target`.
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
    if names(latest.body(), target) {
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

/// The runs that have not ended and pin `target` by the rule an archive stops runs by ([`pins`]). A run that cannot
/// be read is logged and left out, as it is when the archive stops runs (`crate::sharing::sync`).
fn runs(db: &ProjectDb, target: &Resource) -> Result<Vec<UsedByRun>, UsedByError> {
  let mut found: Vec<(NonZeroU32, UsedByRun)> = Vec::new();
  for listed in db.unfinished_runs()? {
    let run = match listed {
      Ok(run) if pins(db, run.snapshot(), target)? => run,
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
