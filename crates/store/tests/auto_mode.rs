//! Manual's reviewer in the project database: the reviewer a person chose, where origin pushed as a run started, and
//! the agent's last messages the reviewer reads. `cargo test -p orchester-store --test auto_mode auto_mode`.

mod support;

use orchester_core::{
  connection::stream::AgentEvent,
  id::{ConnectionId, DecisionConnectionId},
  resource::ModelId,
  run::review::{AutoModeSettings, ReviewerChoice},
  store::ActivityStore,
};
use orchester_store::AutoModeSave;
use support::{at, started_run};

#[test]
fn auto_mode_keeps_the_reviewer_a_person_chose_and_the_runs_origin() {
  let (_repo, mut db, run, _) = started_run();
  assert_eq!(db.auto_mode_settings().expect("read"), AutoModeSettings::default());

  let settings = AutoModeSettings {
    reviewer: Some(ReviewerChoice {
      connection: (ConnectionId::generate()).into(),
      model: ModelId::new("claude-sonnet-5-5").expect("model"),
      effort: Some("high".to_owned()),
      threshold: None,
    }),
  };
  db.save_auto_mode_settings(&settings).expect("save");
  assert_eq!(db.auto_mode_settings().expect("read"), settings);
  // Back to the default reviewer: no connection, model or effort is read.
  let default = AutoModeSettings::default();
  db.save_auto_mode_settings(&default).expect("save");
  assert_eq!(db.auto_mode_settings().expect("read"), default);
  // Where origin pushed as the run started.
  assert_eq!(db.run_origin(run.id()).expect("read"), None);
  db.record_run_origin(run.id(), Some("https://example.invalid/kontur.git"))
    .expect("record");
  assert_eq!(
    db.run_origin(run.id()).expect("read").as_deref(),
    Some("https://example.invalid/kontur.git")
  );
}

#[test]
fn auto_mode_reads_the_agents_last_messages_oldest_first() {
  let (_repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &support::model(), at(10))
    .expect("invocation");
  let events: Vec<_> = (1..=4)
    .map(|number| {
      (
        at(10 + number),
        AgentEvent::Message {
          text: format!("message {number}"),
        },
      )
    })
    .chain([(at(20), AgentEvent::TurnStarted)])
    .collect();
  db.append_activity(&invocation, &events).expect("activity");
  assert_eq!(
    db.recent_agent_messages(run.id(), execution, 3).expect("messages"),
    ["message 2", "message 3", "message 4"]
  );
}

#[test]
fn auto_mode_update_and_compare_and_save_change_what_is_stored() {
  let (_repo, mut db, ..) = started_run();
  let reviewer = ReviewerChoice {
    connection: (ConnectionId::generate()).into(),
    model: ModelId::new("claude-sonnet-5-5").expect("model"),
    effort: Some("high".to_owned()),
    threshold: None,
  };
  let chosen = AutoModeSettings {
    reviewer: Some(reviewer),
  };
  let saved = db.update_auto_mode_settings(|_| chosen.clone()).expect("update");
  assert_eq!(saved, chosen);
  assert_eq!(db.auto_mode_settings().expect("read"), saved);
  // A save from a stale base is refused and changes nothing; one from the stored base is taken.
  let default = AutoModeSettings::default();
  assert_eq!(
    db.save_auto_mode_if_current(&default, &default).expect("save"),
    AutoModeSave::ChangedOutside
  );
  assert_eq!(db.auto_mode_settings().expect("read"), chosen);
  assert_eq!(
    db.save_auto_mode_if_current(&chosen, &default).expect("save"),
    AutoModeSave::Saved
  );
  assert_eq!(db.auto_mode_settings().expect("read"), default);
}

#[test]
fn auto_mode_refuses_a_missing_decision_connection() {
  let (_repo, mut db, ..) = started_run();
  let settings = AutoModeSettings {
    reviewer: Some(ReviewerChoice::decision(
      DecisionConnectionId::generate(),
      ModelId::new("system-one").expect("model"),
      None,
    )),
  };
  assert!(matches!(
    db.save_auto_mode_settings(&settings),
    Err(orchester_store::StoreError::DecisionConnectionNotFound(_))
  ));
  assert_eq!(db.auto_mode_settings().expect("unchanged"), AutoModeSettings::default());
}

#[test]
fn auto_mode_decision_reviewer_round_trips_and_protects_connection_until_switching_back() {
  use orchester_core::{
    connection::{DecisionConnection, DecisionServerUrl},
    name::Name,
    run::review::Probability,
  };
  let (repo, mut db, ..) = started_run();
  let connection = DecisionConnection::new(
    DecisionConnectionId::generate(),
    Name::new("Decision").expect("name"),
    DecisionServerUrl::new("http://localhost:8000/v1").expect("url"),
    None,
  );
  db.create_decision_connection(&connection).expect("connection");
  let settings = AutoModeSettings {
    reviewer: Some(ReviewerChoice::decision(
      connection.id(),
      ModelId::new("jev-latest").expect("model"),
      Some(Probability::new(0.85).expect("threshold")),
    )),
  };
  let default = AutoModeSettings::default();
  assert_eq!(
    db.save_auto_mode_if_current(&default, &settings).expect("choose"),
    AutoModeSave::Saved
  );
  let reopened = orchester_store::ProjectDb::open(repo.path()).expect("reopen");
  assert_eq!(reopened.auto_mode_settings().expect("read"), settings);
  assert_eq!(
    reopened.decision_reviewer_connection().expect("reference"),
    Some(connection.id())
  );
  assert!(matches!(
    db.delete_decision_connection(connection.id()),
    Err(orchester_store::StoreError::DecisionConnectionInUse(_))
  ));
  assert_eq!(
    db.save_auto_mode_if_current(&default, &default).expect("stale"),
    AutoModeSave::ChangedOutside
  );
  db.update_auto_mode_settings(|_| default.clone()).expect("language");
  assert_eq!(db.decision_reviewer_connection().expect("reference cleared"), None);
  db.delete_decision_connection(connection.id())
    .expect("no longer selected");
  assert_eq!(db.auto_mode_settings().expect("default"), default);
}

#[test]
fn decision_review_settings_keep_threshold_and_connection_across_reopen_and_switches() {
  use orchester_core::{
    connection::{DecisionConnection, DecisionServerUrl},
    name::Name,
    run::review::Probability,
  };
  let (repo, mut db, ..) = started_run();
  let connection = DecisionConnection::new(
    DecisionConnectionId::generate(),
    Name::new("Decision").expect("name"),
    DecisionServerUrl::new("http://localhost:8000").expect("URL"),
    None,
  );
  db.create_decision_connection(&connection).expect("connection");
  let settings = AutoModeSettings {
    reviewer: Some(ReviewerChoice::decision(
      connection.id(),
      ModelId::new("jev").expect("model"),
      Some(Probability::new(0.93).expect("threshold")),
    )),
  };
  db.save_auto_mode_settings(&settings).expect("save decision");
  assert_eq!(
    db.decision_reviewer_connection().expect("reference"),
    Some(connection.id())
  );
  drop(db);
  let raw = rusqlite::Connection::open(repo.path().join(".orchester/orchester.db")).expect("raw database");
  let stored: (String, f64) = raw
    .query_row("SELECT model, threshold FROM decision_reviewer_connection", [], |row| {
      Ok((row.get(0)?, row.get(1)?))
    })
    .expect("column settings");
  assert_eq!(stored, ("jev".to_owned(), 0.93));
  // A database written by the base branch has columns but no JSON settings.
  raw
    .execute("UPDATE decision_reviewer_connection SET settings = NULL", [])
    .expect("base settings");
  drop(raw);
  let mut db = orchester_store::ProjectDb::open(repo.path()).expect("reopen");
  assert_eq!(db.auto_mode_settings().expect("read"), settings);
  assert!(db.delete_decision_connection(connection.id()).is_err());
  assert_eq!(
    db.save_auto_mode_if_current(&settings, &AutoModeSettings::default())
      .expect("switch"),
    AutoModeSave::Saved
  );
  assert_eq!(db.decision_reviewer_connection().expect("reference cleared"), None);
  assert_eq!(db.auto_mode_settings().expect("default"), AutoModeSettings::default());
  db.delete_decision_connection(connection.id())
    .expect("unused connection");
}

#[test]
fn decision_reviewer_setting_migrates_the_previously_accepted_schema() {
  use orchester_core::{
    connection::{DecisionConnection, DecisionServerUrl},
    name::Name,
    run::review::Probability,
  };
  use orchester_store::ProjectDb;

  let (repo, mut db, ..) = started_run();
  let connection = DecisionConnection::new(
    DecisionConnectionId::generate(),
    Name::new("Decision").expect("name"),
    DecisionServerUrl::new("http://localhost:8000").expect("URL"),
    None,
  );
  db.create_decision_connection(&connection).expect("connection");
  let settings = AutoModeSettings {
    reviewer: Some(ReviewerChoice::decision(
      connection.id(),
      ModelId::new("jev").expect("model"),
      Some(Probability::new(0.93).expect("threshold")),
    )),
  };
  db.save_auto_mode_settings(&settings).expect("save decision");
  drop(db);
  let current = repo.path().join(".orchester/orchester.db");
  let raw = rusqlite::Connection::open(&current).expect("current database");
  let json: String = raw
    .query_row("SELECT settings FROM decision_reviewer_connection", [], |row| {
      row.get(0)
    })
    .expect("JSON settings");
  drop(raw);

  // a51326fe3 applied settings as migration 122, after the common prefix of 121 migrations.
  let old = support::copied_at_version(&current, 121);
  let raw = rusqlite::Connection::open(old.path().join(".orchester/orchester.db")).expect("old database");
  raw
    .execute_batch("ALTER TABLE decision_reviewer_connection ADD COLUMN settings TEXT; PRAGMA user_version = 122;")
    .expect("previously accepted migration");
  raw
    .execute("UPDATE decision_reviewer_connection SET settings = ?1", [json])
    .expect("previously selected reviewer");
  drop(raw);

  let mut db = ProjectDb::open(old.path()).expect("upgrade previously accepted database");
  assert_eq!(db.auto_mode_settings().expect("preserved reviewer"), settings);
  db.save_auto_mode_settings(&settings).expect("write new columns");
  drop(db);
  let raw = rusqlite::Connection::open(old.path().join(".orchester/orchester.db")).expect("upgraded database");
  let stored: (String, f64) = raw
    .query_row("SELECT model, threshold FROM decision_reviewer_connection", [], |row| {
      Ok((row.get(0)?, row.get(1)?))
    })
    .expect("model and threshold columns migrated");
  assert_eq!(stored, ("jev".to_owned(), 0.93));
  raw
    .prepare("SELECT decision_failure FROM reviewer_usage")
    .expect("review failure column migrated");
  drop(raw);
  let db = ProjectDb::open(old.path()).expect("reopen upgraded database");
  assert_eq!(db.auto_mode_settings().expect("reviewer after reopen"), settings);
}
