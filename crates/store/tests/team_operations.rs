//! Team operations (team authority P2): an operation's id and request recorded before it is sent, one id bound to one
//! request by its digest, open operations listed for recovery, and a settled outcome that never changes.
//!
//! `cargo test -p orchester-store --test team_operations team_operations_`

use std::fs;

use orchester_core::{
  blob::ContentHash,
  id::{OperationId, ProjectId, ServerId},
  name::Name,
  project::{Project, ProjectKey},
  time::Timestamp,
};
use orchester_store::{NewTeamOperation, ProjectDb, StoreError, TeamOperationState, TeamSettlement};
use tempfile::TempDir;

/// A project linked to a server project.
fn linked() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("key"),
    Name::new("Kontur").expect("name"),
  );
  drop(ProjectDb::create(repo.path(), project).expect("create project"));
  rusqlite::Connection::open(repo.path().join(".orchester").join("orchester.db"))
    .expect("open")
    .execute(
      "INSERT INTO sharing (only_row, server_id, project_id, project_key, role, joined_at, event_cursor, \
       last_synced_at) VALUES (1, ?1, ?2, 'ORC', 'member', 1, 0, 1)",
      [ServerId::generate().to_string(), ProjectId::generate().to_string()],
    )
    .expect("link the project");
  let db = ProjectDb::open(repo.path()).expect("open");
  (repo, db)
}

fn digest(digit: char) -> ContentHash {
  ContentHash::new(&digit.to_string().repeat(64)).expect("hash")
}

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

#[test]
fn team_operations_bind_one_id_to_one_request() {
  let (_repo, mut db) = linked();
  let id = OperationId::generate();
  let (first, other) = (digest('a'), digest('b'));
  let new = |digest| NewTeamOperation {
    id,
    kind: "task.patch",
    digest,
    request: "{\"command\":1}",
  };
  let recorded = db.begin_team_operation(new(&first), at(1)).expect("recorded");
  assert_eq!(recorded.state, TeamOperationState::Sent);
  assert_eq!(db.begin_team_operation(new(&first), at(2)).expect("again"), recorded);
  assert!(matches!(
    db.begin_team_operation(new(&other), at(3)),
    Err(StoreError::OperationMismatch(known)) if known == id
  ));
  assert_eq!(db.open_team_operations().expect("open"), [recorded]);
}

#[test]
fn team_operations_stay_with_the_server_project_they_were_recorded_for() {
  let (repo, mut db) = linked();
  let first = db.sharing().expect("read").expect("linked");
  let hash = digest('d');
  let new = NewTeamOperation {
    id: OperationId::generate(),
    kind: "task.create",
    digest: &hash,
    request: "{}",
  };
  let recorded = db.begin_team_operation(new, at(1)).expect("recorded");
  assert_eq!((recorded.server, recorded.project), (first.server, first.project));
  let connection = rusqlite::Connection::open(repo.path().join(".orchester").join("orchester.db")).expect("open");
  connection.execute("DELETE FROM sharing", []).expect("make local");
  assert!(db.open_team_operations().expect("open").is_empty(), "none while local");
  connection
    .execute(
      "INSERT INTO sharing (only_row, server_id, project_id, project_key, role, joined_at, event_cursor, \
       last_synced_at) VALUES (1, ?1, ?2, 'ORC', 'member', 1, 0, 1)",
      [ServerId::generate().to_string(), ProjectId::generate().to_string()],
    )
    .expect("link elsewhere");
  assert!(db.open_team_operations().expect("open").is_empty(), "another project's");
  assert!(matches!(
    db.begin_team_operation(new, at(2)),
    Err(StoreError::OtherServerProject)
  ));
  assert_eq!(
    db.team_operation(new.id).expect("read"),
    Some(recorded),
    "kept as it was"
  );
}

#[test]
fn team_operations_keep_a_settled_outcome() {
  let (_repo, mut db) = linked();
  let id = OperationId::generate();
  let hash = digest('c');
  let new = NewTeamOperation {
    id,
    kind: "claim.acquire",
    digest: &hash,
    request: "{}",
  };
  db.begin_team_operation(new, at(1)).expect("recorded");
  db.settle_team_operation(id, TeamSettlement::Committed { receipt: "{\"r\":1}" }, at(2))
    .expect("committed");
  db.settle_team_operation(id, TeamSettlement::EffectFailed { receipt: "{\"r\":1}" }, at(3))
    .expect("an effect failed");
  assert_eq!(db.open_team_operations().expect("open").len(), 1, "still open");
  assert!(
    db.settle_team_operation(id, TeamSettlement::Committed { receipt: "{\"r\":2}" }, at(4))
      .is_err(),
    "a committed receipt never changes"
  );
  assert!(
    db.settle_team_operation(id, TeamSettlement::Rejected { rejection: "{}" }, at(4))
      .is_err(),
    "a committed operation is never refused"
  );
  db.settle_team_operation(id, TeamSettlement::Completed { receipt: "{\"r\":1}" }, at(5))
    .expect("completed");
  assert!(db.open_team_operations().expect("open").is_empty());
  let kept = db.team_operation(id).expect("read").expect("kept");
  assert_eq!(
    (kept.state, kept.receipt.as_deref()),
    (TeamOperationState::Completed, Some("{\"r\":1}"))
  );
  assert!(
    db.settle_team_operation(id, TeamSettlement::Completed { receipt: "{\"r\":1}" }, at(6))
      .is_err(),
    "a completed operation is settled"
  );
  assert!(matches!(
    db.settle_team_operation(
      OperationId::generate(),
      TeamSettlement::Rejected { rejection: "{}" },
      at(7)
    ),
    Err(StoreError::OperationNotFound(_))
  ));
}
