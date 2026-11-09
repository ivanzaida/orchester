//! What the task's page keeps of a team start apart from its stage (ORC-885c): the runner's last refusal, the mark that
//! a settled start was seen, and the index a task's newest start is read by. The start's own rows are exercised through
//! the workspace's team work (`crates/workspace/tests/team_work/start_notices.rs`).
//!
//! `cargo test -p orchester-store --test team_start_notes team_start_notes_`

mod support;

use std::{fs, path::Path};

use orchester_core::{
  id::{OperationId, ProjectId, ServerId, TaskId},
  name::Name,
  project::{Project, ProjectKey},
  sharing::TaskRevision,
  task::identity::TeamTaskRef,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError, TeamStartStage};
use support::{at, copied_at_version};

/// The schema before `team_start_notes`. Migrations are append-only, so a later one still applies on open.
const BEFORE_START_NOTES: u32 = 160;

/// A project linked to a server project, with the start `operation` of a team task recorded at `Requested`.
fn started() -> (tempfile::TempDir, ProjectDb, OperationId, TaskId) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("key"),
    Name::new("Kontur").expect("name"),
  );
  drop(ProjectDb::create(repo.path(), project).expect("create project"));
  let (server, team) = (ServerId::generate(), ProjectId::generate());
  rusqlite::Connection::open(database(repo.path()))
    .expect("open")
    .execute(
      "INSERT INTO sharing (only_row, server_id, project_id, project_key, role, joined_at, event_cursor, \
       last_synced_at) VALUES (1, ?1, ?2, 'ORC', 'member', 1, 0, 1)",
      [server.to_string(), team.to_string()],
    )
    .expect("link the project");
  let mut db = ProjectDb::open(repo.path()).expect("open");
  let identity = TeamTaskRef {
    server,
    project: team,
    task: TaskId::generate(),
  };
  let task = db
    .record_team_task(identity, TaskRevision::try_from(1).expect("a revision"))
    .expect("the task");
  let operation = OperationId::generate();
  db.begin_team_start((operation, task), "{}", at(1)).expect("a start");
  (repo, db, operation, task)
}

fn database(root: &Path) -> std::path::PathBuf {
  root.join(".orchester").join("orchester.db")
}

fn updated_at(root: &Path, operation: OperationId) -> i64 {
  rusqlite::Connection::open(database(root))
    .expect("open")
    .query_row(
      "SELECT updated_at FROM team_starts WHERE operation_id = ?1",
      [operation.to_string()],
      |row| row.get(0),
    )
    .expect("the start's row")
}

#[test]
fn team_start_notes_an_index_reads_a_task_s_starts_and_a_missing_start_keeps_nothing() {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("key"),
    Name::new("Kontur").expect("name"),
  );
  let mut db = ProjectDb::create(repo.path(), project).expect("create project");

  let connection = rusqlite::Connection::open(repo.path().join(".orchester").join("orchester.db")).expect("open");
  let indexed: String = connection
    .query_row(
      "SELECT sql FROM sqlite_master WHERE type = 'index' AND name = 'team_starts_task'",
      [],
      |row| row.get(0),
    )
    .expect("the index of a task's starts");
  assert!(indexed.contains("(task_id, created_at)"), "{indexed}");

  let operation = OperationId::generate();
  assert!(matches!(
    db.mark_team_start_seen(operation, Timestamp::from_unix_millis(1)),
    Err(StoreError::TeamStartNotFound(found)) if found == operation
  ));
  assert!(matches!(
    db.keep_team_start_runner_refusal(operation, Some("{}"), Timestamp::from_unix_millis(1)),
    Err(StoreError::TeamStartNotFound(found)) if found == operation
  ));
}

#[test]
fn team_start_notes_only_a_settled_start_is_seen_and_only_an_open_one_keeps_a_runner_refusal() {
  let (repo, mut db, operation, task) = started();
  assert_eq!(updated_at(repo.path(), operation), 1);

  assert!(matches!(
    db.mark_team_start_seen(operation, at(2)),
    Err(StoreError::TeamStartStage { operation: found, stage: "requested" }) if found == operation
  ));
  db.keep_team_start_runner_refusal(operation, Some("{\"busy\":true}"), at(3))
    .expect("an open start keeps the refusal");
  let kept = db.latest_team_start(task).expect("read").expect("a start");
  assert_eq!(kept.runner_refusal.as_deref(), Some("{\"busy\":true}"));
  assert!(!kept.seen);
  assert_eq!(
    updated_at(repo.path(), operation),
    3,
    "a change moves the start's last change"
  );
  db.keep_team_start_runner_refusal(operation, None, at(4))
    .expect("the runner did not refuse this time");
  assert_eq!(
    db.latest_team_start(task)
      .expect("read")
      .expect("a start")
      .runner_refusal,
    None
  );

  db.refuse_team_start(operation, "{}", at(5)).expect("refused");
  assert!(matches!(
    db.keep_team_start_runner_refusal(operation, Some("{}"), at(6)),
    Err(StoreError::TeamStartStage { operation: found, stage: "refused" }) if found == operation
  ));
  db.mark_team_start_seen(operation, at(7))
    .expect("a settled start is seen");
  db.mark_team_start_seen(operation, at(8))
    .expect("seen again changes nothing");
  let seen = db.latest_team_start(task).expect("read").expect("a start");
  assert_eq!((seen.stage, seen.seen), (TeamStartStage::Refused, true));
}

#[test]
fn team_start_notes_migration_keeps_a_start_recorded_before_it_and_gives_it_no_note() {
  let (repo, mut db, operation, task) = started();
  db.refuse_team_start(operation, "{\"code\":\"claim_lost\"}", at(5))
    .expect("refused");
  let open = OperationId::generate();
  db.begin_team_start((open, task), "{\"second\":1}", at(6))
    .expect("a second start");
  drop(db);

  let old = copied_at_version(&database(repo.path()), BEFORE_START_NOTES);
  {
    let raw = rusqlite::Connection::open(database(old.path())).expect("the old database");
    let has_note: bool = raw
      .query_row(
        "SELECT EXISTS (SELECT 1 FROM pragma_table_info('team_starts') WHERE name = 'runner_refusal')",
        [],
        |row| row.get(0),
      )
      .expect("columns");
    assert!(!has_note, "before the migration a start keeps no runner refusal");
    let rows: i64 = raw
      .query_row("SELECT COUNT(*) FROM team_starts", [], |row| row.get(0))
      .expect("count");
    assert_eq!(rows, 2, "the starts were recorded before the migration");
  }

  let migrated = ProjectDb::open(old.path()).expect("migrate the existing project");
  let newest = migrated.latest_team_start(task).expect("read").expect("a start");
  assert_eq!(newest.operation, open, "the newest start is the one recorded last");
  assert_eq!((newest.runner_refusal, newest.seen), (None, false));
  let first = migrated.team_start(operation).expect("read").expect("the first start");
  assert_eq!(first.stage, TeamStartStage::Refused);
  assert_eq!(first.refusal.as_deref(), Some("{\"code\":\"claim_lost\"}"));
  assert!(!first.seen, "nothing was seen before the mark existed");
}
