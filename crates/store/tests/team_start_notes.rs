//! What the task's page keeps of a team start apart from its stage (ORC-885c): the runner's last refusal, the mark that
//! a settled start was seen, and the index a task's newest start is read by. The start's own rows are exercised through
//! the workspace's team work (`crates/workspace/tests/team_work/start_notices.rs`).
//!
//! `cargo test -p orchester-store --test team_start_notes team_start_notes_`

use std::fs;

use orchester_core::{
  id::{OperationId, ProjectId},
  name::Name,
  project::{Project, ProjectKey},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

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
    db.keep_team_start_runner_refusal(operation, Some("{}")),
    Err(StoreError::TeamStartNotFound(found)) if found == operation
  ));
}
