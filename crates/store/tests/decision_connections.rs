mod support;
use orchester_core::{
  connection::{
    DecisionCheck, DecisionCheckResult, DecisionConnection, DecisionServerUrl, DecisionTest, DecisionThresholdFigures,
  },
  id::{DecisionConnectionId, ProjectId, SecretId},
  name::Name,
  project::{Project, ProjectKey},
  resource::ModelId,
  run::review::Probability,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use rusqlite::Connection as Sqlite;
use tempfile::TempDir;

fn fixture() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary repository");
  std::fs::create_dir(repo.path().join(".git")).expect("git directory");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("key"),
    name("Project"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("create project");
  (repo, db)
}

fn name(value: &str) -> Name {
  Name::new(value).expect("name")
}

fn model() -> ModelId {
  ModelId::new("jev").expect("model")
}

fn connection() -> DecisionConnection {
  DecisionConnection::new(
    DecisionConnectionId::generate(),
    name("Decision server"),
    DecisionServerUrl::new("http://localhost:8000").expect("URL"),
    Some(SecretId::generate()),
  )
  .with_models(vec![model()])
}

fn test_result() -> DecisionTest {
  DecisionTest {
    at: Timestamp::from_unix_millis(2000),
    model: model(),
    figures: [0.8, 0.9]
      .into_iter()
      .map(|threshold| DecisionThresholdFigures {
        threshold: Probability::new(threshold).expect("probability"),
        approves_on_its_own: 70,
        comes_to_person: 30,
        approves_earlier_reviewer_asks: 12,
        requests_tested: 100,
        median_ms: 42,
        tokens: 1024,
      })
      .collect(),
  }
}

#[test]
fn decision_connections_round_trip_and_edit_keep_checks_tests_and_secret_reference_only() {
  let (repo, mut db) = fixture();
  let mut value = connection();
  db.create_decision_connection(&value).expect("add");
  assert_eq!(db.decision_connection(value.id()).expect("read"), Some(value.clone()));
  value.record_test(test_result());
  let results = [
    DecisionCheckResult::Connected,
    DecisionCheckResult::KeyRefused,
    DecisionCheckResult::NothingAnswers,
    DecisionCheckResult::NotSystemOneApi,
    DecisionCheckResult::ModelGone { model: model() },
  ];
  for result in results {
    value.record_check(DecisionCheck {
      at: Timestamp::from_unix_millis(1000),
      result,
    });
    value.rename(name("Renamed server"));
    db.save_decision_connection(&value).expect("edit");
    let reopened = ProjectDb::open(repo.path()).expect("reopen");
    assert_eq!(reopened.decision_connections().expect("list"), vec![value.clone()]);
  }
  let raw = Sqlite::open(repo.path().join(".orchester/orchester.db")).expect("raw database");
  let settings: String = raw
    .query_row("SELECT settings FROM decision_connections", [], |row| row.get(0))
    .expect("settings");
  let stored: serde_json::Value = serde_json::from_str(&settings).expect("JSON");
  assert_eq!(stored["body"]["key"], value.key().expect("reference").to_string());
  let keys = stored["body"]
    .as_object()
    .expect("object")
    .keys()
    .map(String::as_str)
    .collect::<Vec<_>>();
  assert_eq!(keys, ["base_url", "key", "last_check", "last_test", "models"]);
  value.set_server(DecisionServerUrl::new("https://example.com").expect("URL"), None);
  db.save_decision_connection(&value).expect("change server");
  let stored = db.decision_connection(value.id()).expect("read").expect("exists");
  assert_eq!(stored, value);
  assert!(stored.last_check().is_none() && stored.last_test().is_none() && stored.models().is_empty());
  db.delete_decision_connection(value.id()).expect("delete unused");
  assert!(db.decision_connections().expect("list").is_empty());
}

#[test]
fn decision_connections_migrate_database_without_table() {
  let (repo, db) = fixture();
  drop(db);
  // Migration 120 introduces decision connections; build its actual predecessor schema.
  let repo = support::copied_at_version(&repo.path().join(".orchester/orchester.db"), 119);
  let mut reopened = ProjectDb::open(repo.path()).expect("migrate");
  assert!(reopened.decision_connections().expect("empty").is_empty());
  assert_eq!(reopened.project().name(), &name("Project"));
  let value = connection();
  reopened.create_decision_connection(&value).expect("new table writable");
  assert_eq!(reopened.decision_connection(value.id()).expect("read"), Some(value));
}

#[test]
fn decision_connections_delete_refused_while_reviewer_uses_it() {
  let (repo, mut db) = fixture();
  let value = connection();
  db.create_decision_connection(&value).expect("add");
  db.save_decision_reviewer_connection(Some(value.id()))
    .expect("reviewer selects");
  let mut other = ProjectDb::open(repo.path()).expect("other writer");
  assert_eq!(
    other.decision_reviewer_connection().expect("selection persisted"),
    Some(value.id())
  );
  let error = other.delete_decision_connection(value.id()).expect_err("in use");
  assert!(error.to_string().contains("the reviewer uses it"));
  assert!(matches!(error, StoreError::DecisionConnectionInUse(id) if id == value.id()));
  assert_eq!(
    db.decision_connection(value.id()).expect("unchanged"),
    Some(value.clone())
  );
  db.save_decision_reviewer_connection(None).expect("stop using");
  other.delete_decision_connection(value.id()).expect("delete");
  assert!(matches!(
    db.save_decision_reviewer_connection(Some(value.id())),
    Err(StoreError::DecisionConnectionNotFound(_))
  ));
}

#[test]
fn decision_connections_reject_duplicate_names_missing_edits_and_corrupt_data() {
  let (repo, mut db) = fixture();
  let value = connection();
  assert!(matches!(
    db.save_decision_connection(&value),
    Err(StoreError::DecisionConnectionNotFound(_))
  ));
  db.create_decision_connection(&value).expect("add");
  assert!(matches!(
    db.create_decision_connection(&connection()),
    Err(StoreError::DuplicateConnectionName(_))
  ));
  let raw = Sqlite::open(repo.path().join(".orchester/orchester.db")).expect("raw database");
  raw
    .execute(
      "UPDATE decision_connections SET settings = json_set(settings, '$.body.base_url', 'invalid')",
      [],
    )
    .expect("damage");
  assert!(matches!(db.decision_connections(), Err(StoreError::Corrupt { .. })));
}

#[test]
fn decision_connections_check_save_refuses_interleaved_edits() {
  let (repo, mut db) = fixture();
  let original = connection();
  db.create_decision_connection(&original).expect("add");
  let mut editor = ProjectDb::open(repo.path()).expect("other writer");
  for change in 0..3 {
    let loaded = db
      .decision_connection(original.id())
      .expect("read")
      .expect("connection");
    let mut checked = loaded.clone();
    checked.record_check(DecisionCheck {
      at: Timestamp::now(),
      result: DecisionCheckResult::Connected,
    });
    let mut edited = loaded.clone();
    match change {
      0 => edited.rename(name("Concurrent rename")),
      1 => edited.set_server(
        DecisionServerUrl::new("https://changed.example/v1").expect("URL"),
        loaded.key(),
      ),
      _ => edited.set_server(loaded.base_url().clone(), Some(SecretId::generate())),
    }
    editor.save_decision_connection(&edited).expect("edit after check read");
    assert!(
      !db
        .save_decision_check_if_unchanged(&loaded, &checked)
        .expect("conditional write")
    );
    assert_eq!(
      db.decision_connection(original.id()).expect("read"),
      Some(edited.clone())
    );
    let mut fresh = edited.clone();
    fresh.record_check(DecisionCheck {
      at: Timestamp::now(),
      result: DecisionCheckResult::Connected,
    });
    assert!(
      db.save_decision_check_if_unchanged(&edited, &fresh)
        .expect("unchanged write")
    );
    assert_eq!(db.decision_connection(original.id()).expect("read"), Some(fresh));
  }
}

#[test]
fn decision_connections_edit_save_atomically_refuses_interleaved_settings_and_name_changes() {
  let (repo, mut db) = fixture();
  let original = connection();
  db.create_decision_connection(&original).expect("add");
  let mut other = ProjectDb::open(repo.path()).expect("other writer");
  for change in 0..4 {
    let expected = db
      .decision_connection(original.id())
      .expect("read")
      .expect("connection");
    let mut edited = expected.clone();
    edited.rename(name("My rename"));
    let mut concurrent = expected.clone();
    match change {
      0 => concurrent.rename(name("Outside rename")),
      1 => concurrent.set_server(
        DecisionServerUrl::new("https://changed.example/v1").expect("url"),
        expected.key(),
      ),
      2 => concurrent.set_server(expected.base_url().clone(), Some(SecretId::generate())),
      _ => concurrent.record_models(vec![ModelId::new("new-model").expect("model")]),
    }
    other.save_decision_connection(&concurrent).expect("concurrent edit");
    assert!(
      !db
        .save_decision_connection_if_unchanged(&expected, &edited)
        .expect("conditional save")
    );
    assert_eq!(
      db.decision_connection(original.id()).expect("read"),
      Some(concurrent.clone())
    );
    let mut fresh = concurrent.clone();
    fresh.rename(name("Fresh rename"));
    assert!(
      db.save_decision_connection_if_unchanged(&concurrent, &fresh)
        .expect("fresh save")
    );
    assert_eq!(db.decision_connection(original.id()).expect("read"), Some(fresh));
  }
}
