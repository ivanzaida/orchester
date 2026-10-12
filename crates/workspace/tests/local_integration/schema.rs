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
