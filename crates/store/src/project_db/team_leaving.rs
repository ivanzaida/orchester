//! Leaving the team (`docs/design/team-configuration.md`, edge case 13): the project keeps no copy of the team's
//! configuration. Every resource marked as the server's goes with its drafts, its cached versions and everything that
//! names it; the project's own resources stay as they are, each kind keeping one resource to a slug again.

use orchester_core::{
  name::Name,
  resource::{Resource, Slug},
};
use rusqlite::{Transaction, params};

use super::resources::{kind_text, load_resources};
use crate::{StoreError, rows::corrupt};

/// Tables that hold rows of one team resource by its `resource_id`, deleted before the resource itself.
const BY_RESOURCE: [&str; 5] = [
  "resource_drafts",
  "removed_resources",
  "repository_imports",
  "automation_drafts",
  "resource_imports",
];

/// Deletes every team resource of the project and what the cache kept of it. The tasks that named a team pipeline
/// name none afterwards.
pub(super) fn remove_team_configuration(transaction: &Transaction<'_>) -> Result<(), StoreError> {
  transaction.execute_batch(
    "CREATE TEMP TABLE team_resources AS SELECT id FROM resources WHERE sharing = 'shared';
     UPDATE tasks SET pipeline_id = NULL WHERE pipeline_id IN (SELECT id FROM team_resources);",
  )?;
  let guards = delete_guards(transaction)?;
  for (name, _) in &guards {
    transaction.execute_batch(&format!("DROP TRIGGER \"{name}\""))?;
  }
  let removed = delete_rows(transaction);
  // The guards come back whether or not the delete worked, so the transaction never commits without them.
  for (_, sql) in &guards {
    transaction.execute_batch(sql)?;
  }
  removed?;
  transaction.execute_batch("DROP TABLE team_resources")?;
  Ok(())
}

fn delete_rows(transaction: &Transaction<'_>) -> Result<(), StoreError> {
  transaction.execute_batch(
    "DELETE FROM shared_publishes;
     DELETE FROM waiting_versions WHERE version_id IN (SELECT id FROM resource_versions
       WHERE resource_id IN (SELECT id FROM team_resources));
     DELETE FROM shared_version_references WHERE version_id IN (SELECT id FROM resource_versions
       WHERE resource_id IN (SELECT id FROM team_resources));
     DELETE FROM shared_version_bodies WHERE version_id IN (SELECT id FROM resource_versions
       WHERE resource_id IN (SELECT id FROM team_resources));
     DELETE FROM resource_versions WHERE resource_id IN (SELECT id FROM team_resources);
     DELETE FROM project_default_pipeline WHERE pipeline_id IN (SELECT id FROM team_resources);",
  )?;
  for table in BY_RESOURCE {
    transaction.execute_batch(&format!(
      "DELETE FROM {table} WHERE resource_id IN (SELECT id FROM team_resources)"
    ))?;
  }
  transaction.execute_batch("DELETE FROM resources WHERE id IN (SELECT id FROM team_resources)")?;
  Ok(())
}

/// The triggers that keep published versions from being deleted, with their definitions, to put back afterwards.
fn delete_guards(transaction: &Transaction<'_>) -> Result<Vec<(String, String)>, StoreError> {
  let mut statement = transaction.prepare(
    "SELECT name, sql FROM sqlite_master WHERE type = 'trigger' AND tbl_name = 'resource_versions' \
     AND upper(sql) LIKE '%BEFORE DELETE%'",
  )?;
  let guards = statement
    .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
    .collect::<Result<Vec<_>, _>>()?;
  Ok(guards)
}

/// Gives every resource that shares its kind's slug with an older one a free slug of its kind, the way a new resource
/// picks one (`-2`, `-3`, …): a team project lets a team resource, or a copy of a pipeline of this computer's own waiting
/// for the server, share the slug of the local one it came from, and the copy that never reached the server is a local
/// resource once the project is not the team's. Every kind is held to one resource to a slug, archived ones included.
pub(super) fn separate_slugs(transaction: &Transaction<'_>) -> Result<(), StoreError> {
  let later = load_resources(
    transaction,
    "AS later WHERE EXISTS (SELECT 1 FROM resources AS earlier WHERE earlier.kind = later.kind \
     AND earlier.slug = later.slug AND (earlier.created_at, earlier.rowid) < (later.created_at, later.rowid)) \
     ORDER BY later.created_at, later.rowid",
    [],
  )?;
  for resource in later {
    let taken: Vec<Slug> = load_resources(transaction, "WHERE kind = ?1", [kind_text(resource.kind())])?
      .iter()
      .map(|other| other.slug().clone())
      .collect();
    let named = Name::new(resource.slug().as_str()).map_err(|error| corrupt("resource slug", error))?;
    let free = Slug::unique_for(&named, resource.slug(), &taken);
    rename(transaction, &resource, &free)?;
  }
  Ok(())
}

fn rename(transaction: &Transaction<'_>, resource: &Resource, free: &Slug) -> Result<(), StoreError> {
  transaction.execute(
    "UPDATE resources SET slug = ?2 WHERE id = ?1",
    params![resource.id().to_string(), free.as_str()],
  )?;
  tracing::info!(
    resource = %resource.id(),
    slug = free.as_str(),
    "a resource took a free slug as its project left the team"
  );
  Ok(())
}
