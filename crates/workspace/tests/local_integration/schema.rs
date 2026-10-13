use orchester_core::{
  blob::ContentHash,
  integration::local::{Checkout, LocalIntegrationStore, LocalOperation, LocalPath, LocalRecoveryStore, RecoveryLayer},
};
use orchester_exec::local_git::digest;
use orchester_store::ProjectDb;

use super::support::{Fixture, git};

#[test]
fn local_integration_draft_migration_preserves_earlier_retained_originals_and_journal() {
  let fixture = Fixture::new(false);
  let checkout = Checkout {
    root: fixture.root().to_str().expect("root").to_owned(),
    git_dir: fixture.root().join(".git").to_str().expect("git").to_owned(),
    branch: "main".into(),
    head: git(fixture.root(), &["rev-parse", "HEAD"]),
    index: ContentHash::new(&"a".repeat(64)).expect("hash"),
    ignored: ContentHash::new(&"b".repeat(64)).expect("hash"),
    files: Vec::new(),
    operation: None,
  };
  let operation = LocalOperation::new(fixture.run, checkout).expect("operation");
  let path = LocalPath::new("original.txt").expect("path");
  let original = b"earlier recovery bytes";
  let mut db = ProjectDb::open(fixture.root()).expect("db");
  assert!(db.save_local_operation(&operation, None).expect("save"));
  db.retain_local_content(fixture.run, &path, RecoveryLayer::Working, &digest(original), original)
    .expect("original");
  drop(db);
  let connection = rusqlite::Connection::open(fixture.root().join(".orchester/orchester.db")).expect("private db");
  let version: u32 = connection
    .pragma_query_value(None, "user_version", |row| row.get(0))
    .expect("version");
  connection.execute_batch("
    ALTER TABLE local_recovery_content RENAME TO recovery_fixture_current;
    CREATE TABLE local_recovery_content (
      run_id TEXT NOT NULL REFERENCES local_integrations(run_id), path TEXT NOT NULL,
      layer TEXT NOT NULL CHECK(layer IN ('head','staged','working','task_ancestor','task_current','task_incoming','working_base','result')),
      digest TEXT NOT NULL, content BLOB NOT NULL, PRIMARY KEY(run_id,path,layer)
    ) STRICT;
    INSERT INTO local_recovery_content SELECT * FROM recovery_fixture_current;
    DROP TABLE recovery_fixture_current;
  ").expect("earlier exact recovery schema");
  connection
    .pragma_update(None, "user_version", version - 1)
    .expect("previous schema");
  drop(connection);
  let mut db = ProjectDb::open(fixture.root()).expect("forward migration");
  assert_eq!(db.local_operation(fixture.run).expect("journal"), Some(operation));
  assert_eq!(
    db.local_content(fixture.run, &path, RecoveryLayer::Working)
      .expect("original"),
    Some(original.to_vec())
  );
  let draft = b"unfinished markers and raw destination";
  let draft_path = LocalPath::new("editor-drafts/private-fixture").expect("path");
  db.retain_local_content(
    fixture.run,
    &draft_path,
    RecoveryLayer::EditorDraft,
    &digest(draft),
    draft,
  )
  .expect("separate candidate");
  assert_eq!(
    db.local_content(fixture.run, &path, RecoveryLayer::Working)
      .expect("still original"),
    Some(original.to_vec())
  );
}

#[test]
fn local_integration_master_account_schema_upgrade_keeps_runs_and_adds_durable_journal() {
  use orchester_core::{integration::local::LocalPhase, store::RunStore};

  let fixture = Fixture::new(false);
  let before = ProjectDb::open(fixture.root())
    .expect("project")
    .run(fixture.run)
    .expect("run")
    .expect("recorded run");
  let connection = rusqlite::Connection::open(fixture.root().join(".orchester/orchester.db")).expect("private db");
  let version: u32 = connection
    .pragma_query_value(None, "user_version", |row| row.get(0))
    .expect("version");
  // Master has the account migration; these two unshipped integration migrations follow it.
  connection
    .execute_batch(
      "DROP TABLE local_integration_process;
       DROP TABLE local_recovery_content;
       DROP TABLE local_integrations;",
    )
    .expect("master schema without integration tables");
  connection
    .pragma_update(None, "user_version", version - 2)
    .expect("master schema version");
  drop(connection);

  let db = ProjectDb::open(fixture.root()).expect("upgrade from master");
  assert_eq!(db.run(fixture.run).expect("run"), Some(before.clone()));
  assert!(db.local_operation(fixture.run).expect("journal").is_none());
  drop(db);
  let mut session = fixture.open();
  session.prepare_clean().expect("prepare real checkout");
  let operation = session.operation().clone();
  assert_eq!(operation.phase(), LocalPhase::Prepared);
  drop(session);

  let db = ProjectDb::open(fixture.root()).expect("restart");
  assert_eq!(db.run(fixture.run).expect("run"), Some(before));
  assert_eq!(
    db.local_operation(fixture.run).expect("durable journal"),
    Some(operation)
  );
}
