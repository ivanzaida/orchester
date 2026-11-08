//! The migration that names what an archive stopped a run for (ORC-838): a database from before it, with a stop
//! recorded as only its resource's name, opens with the row kept and reads back with no resource, kind or archiver; a
//! row that names only part of the three is corrupt, not read as a stop of an unknown resource; and a stop recorded
//! afterwards reads back whole.
//! `cargo test -p orchester-store --test archive_stops_migration archive_stops_migration`.

mod support;

use orchester_core::{id::ResourceId, resource::ResourceKind};
use orchester_store::{ArchiveStop, Archived, ProjectDb, StoreError};
use rusqlite::Connection;
use support::{at, copied_at_version, name, started_run};

/// The migrations after the one that added the three columns: none yet. A migration appended later raises it, so the
/// rewound schema stays the one before this migration.
const LATER_MIGRATIONS: u32 = 0;

fn latest_version(database: &std::path::Path) -> u32 {
  Connection::open(database)
    .expect("open")
    .pragma_query_value(None, "user_version", |row| row.get(0))
    .expect("user_version")
}

#[test]
fn archive_stops_migration_keeps_a_stop_recorded_before_it_without_a_resource_and_refuses_a_partial_row() {
  let (repo, db, run, _) = started_run();
  let task = run.task();
  drop(db);
  let current = repo.path().join(".orchester").join("orchester.db");
  let before = latest_version(&current) - LATER_MIGRATIONS - 1;
  let old = copied_at_version(&current, before);
  let old_file = old.path().join(".orchester").join("orchester.db");
  {
    let raw = Connection::open(&old_file).expect("the old database");
    let columns: i64 = raw
      .query_row(
        "SELECT COUNT(*) FROM pragma_table_info('archive_stops') WHERE name IN ('resource_id', 'resource_kind', 'archived_by')",
        [],
        |row| row.get(0),
      )
      .expect("columns");
    assert_eq!(columns, 0, "before the migration a stop names no resource");
    raw
      .execute(
        "INSERT INTO archive_stops (run_id, task_id, resource_name, stopped_at) VALUES (?1, ?2, 'pencil-design', 1000)",
        [run.id().to_string(), task.to_string()],
      )
      .expect("a stop recorded before");
    raw.close().expect("closed");
  }

  let migrated = ProjectDb::open(old.path()).expect("migrate the existing project");
  let stops = migrated.archive_stops().expect("the stops");
  assert_eq!(stops.len(), 1, "the row is kept");
  assert_eq!(stops[0].resource.as_str(), "pencil-design");
  assert_eq!(stops[0].archived, None, "it names no resource, kind or archiver");
  assert_eq!(
    migrated.archive_stop_of(run.id()).expect("read"),
    Some(stops[0].clone())
  );
  drop(migrated);

  // Part of the three is no stop of an unknown resource: it is corrupt.
  let raw = Connection::open(&old_file).expect("the migrated database");
  raw
    .execute("UPDATE archive_stops SET archived_by = 'dana-k'", [])
    .expect("an archiver alone");
  let partial = ProjectDb::open(old.path()).expect("open").archive_stops();
  assert!(
    matches!(partial, Err(StoreError::Corrupt { .. })),
    "an archiver without a resource is corrupt: {partial:?}"
  );
  raw
    .execute(
      "UPDATE archive_stops SET resource_id = ?1, archived_by = NULL",
      [ResourceId::generate().to_string()],
    )
    .expect("a resource id alone");
  let partial = ProjectDb::open(old.path()).expect("open").archive_stops();
  assert!(
    matches!(partial, Err(StoreError::Corrupt { .. })),
    "a resource without its kind is corrupt: {partial:?}"
  );
}

#[test]
fn archive_stops_migration_a_stop_recorded_afterwards_reads_back_whole() {
  let (repo, mut db, run, _) = started_run();
  let archived = Archived {
    id: ResourceId::generate(),
    kind: ResourceKind::McpServer,
    by: Some("dana-k".to_owned()),
  };
  let stop = ArchiveStop {
    run: run.id(),
    task: run.task(),
    resource: name("pencil-design"),
    stopped_at: at(2000),
    archived: Some(archived),
  };
  db.record_archive_stop(&stop).expect("recorded");
  drop(db);
  let reopened = ProjectDb::open(repo.path()).expect("reopened");
  assert_eq!(reopened.archive_stop_of(run.id()).expect("read"), Some(stop));
}
