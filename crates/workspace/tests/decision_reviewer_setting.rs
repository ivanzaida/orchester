//! Workspace reviewer selection, using recorded local fixtures without inference or credentials.
#[path = "decision_reviewer_setting/tools.rs"]
mod tools;
use std::fs;

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, DecisionCheck, DecisionCheckResult, DecisionConnection, DecisionServerUrl,
    DecisionTest, DecisionThresholdFigures,
  },
  id::{ConnectionId, DecisionConnectionId, ProjectId},
  name::Name,
  project::{Project, ProjectKey},
  resource::ModelId,
  run::review::{AutoModeSettings, ReviewerChoice, ReviewerKind, ReviewerUnavailable},
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::project_settings::{self, decision_reviewer as reviewer};

fn project() -> (tempfile::TempDir, ProjectDb) {
  let scratch = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/decision-reviewer/tests");
  fs::create_dir_all(&scratch).expect("scratch");
  let repo = tempfile::tempdir_in(scratch).expect("temporary project");
  fs::create_dir(repo.path().join(".git")).expect("git folder");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("DEC").expect("key"),
    Name::new("Reviewer").expect("name"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("database");
  (repo, db)
}

fn model() -> ModelId {
  ModelId::new("jev-latest").expect("model")
}

fn connection(url: &str) -> DecisionConnection {
  let mut connection = DecisionConnection::new(
    DecisionConnectionId::generate(),
    Name::new(if url.contains("localhost") {
      "Local decision"
    } else {
      "Decision"
    })
    .expect("name"),
    DecisionServerUrl::new(url).expect("url"),
    None,
  )
  .with_models(vec![model()]);
  connection.record_check(DecisionCheck {
    at: Timestamp::now(),
    result: DecisionCheckResult::Connected,
  });
  connection
}

fn settings(connection: &DecisionConnection, threshold: &str) -> AutoModeSettings {
  AutoModeSettings {
    reviewer: Some(reviewer::choose(connection, model(), threshold).expect("choice")),
  }
}

#[test]
fn decision_reviewer_setting_threshold_and_model_validation() {
  let local = connection("http://localhost:8000/v1");
  for valid in ["0.01", "0.99", "0.80", "0.1"] {
    assert!(reviewer::choose(&local, model(), valid).is_ok());
  }
  for invalid in ["0.00", "1.00", "0.999", "0.801", "NaN", "-0.1", "0", ""] {
    assert!(reviewer::choose(&local, model(), invalid).is_err(), "{invalid}");
  }
  assert!(reviewer::choose(&local, ModelId::new("gone").expect("model"), "0.80").is_err());
}

#[test]
fn decision_reviewer_setting_saves_local_and_requires_hosted_confirmation_only_when_choosing() {
  let (repo, mut db) = project();
  let local = connection("http://localhost:8000/v1");
  let hosted = connection("https://review.example/v1");
  db.create_decision_connection(&local).expect("local");
  db.create_decision_connection(&hosted).expect("hosted");
  let base = AutoModeSettings::default();
  let local_choice = settings(&local, "0.80");
  assert_eq!(
    project_settings::save_auto_mode_in(repo.path(), &base, &local_choice, false, Some(&local))
      .expect_err("new local reviewer needs permission consent")
      .key,
    "projectSettings.personOnly"
  );
  project_settings::save_auto_mode_in(repo.path(), &base, &local_choice, true, Some(&local)).expect("local");
  let hosted_choice = settings(&hosted, "0.80");
  assert_eq!(
    project_settings::save_auto_mode_in(repo.path(), &local_choice, &hosted_choice, false, Some(&hosted))
      .expect_err("confirmation")
      .key,
    "projectSettings.personOnly"
  );
  let question =
    reviewer::choosing_hosted(&local_choice, &hosted_choice, std::slice::from_ref(&hosted)).expect("question");
  assert_eq!(question.host, "review.example");
  assert!(!question.on_this_computer);
  assert_eq!(question.sends.len(), 4);
  project_settings::save_auto_mode_in(repo.path(), &local_choice, &hosted_choice, true, Some(&hosted))
    .expect("person confirms");
  for value in ["0.90", "0.80", "0.10"] {
    let current = db.auto_mode_settings().expect("current");
    let asked = settings(&hosted, value);
    assert!(reviewer::choosing_hosted(&current, &asked, std::slice::from_ref(&hosted)).is_none());
    project_settings::save_auto_mode_in(repo.path(), &current, &asked, false, Some(&hosted)).expect("threshold edit");
  }
  let language = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Claude").expect("name"),
    AgentProvider::ClaudeCodeCli,
    std::env::current_exe().expect("program"),
    Timestamp::now(),
  )
  .expect("agent");
  db.create_connection(&language.into_connection()).expect("language");
  let current = db.auto_mode_settings().expect("current");
  project_settings::save_auto_mode_in(repo.path(), &current, &base, false, None).expect("back to language");
  assert_eq!(db.auto_mode_settings().expect("saved"), base);
  assert!(
    project_settings::save_auto_mode_in(repo.path(), &current, &hosted_choice, true, Some(&hosted)).is_err(),
    "stale baseline"
  );
}

#[test]
fn decision_reviewer_setting_read_model_keeps_destination_figures_and_unavailable_choice() {
  let (repo, mut db) = project();
  let mut hosted = connection("https://review.example/v1");
  let figures = DecisionThresholdFigures {
    threshold: reviewer::threshold("0.80").expect("threshold"),
    approves_on_its_own: 70,
    comes_to_person: 30,
    approves_earlier_reviewer_asks: 5,
    requests_tested: 100,
    median_ms: 12,
    tokens: 500,
  };
  hosted.record_test(DecisionTest {
    at: Timestamp::now(),
    model: model(),
    figures: vec![figures.clone()],
  });
  db.create_decision_connection(&hosted).expect("connection");
  let chosen = settings(&hosted, "0.80");
  db.save_auto_mode_settings(&chosen).expect("settings");
  let row = reviewer::read(&db).expect("read");
  assert_eq!(row.kind, Some(ReviewerKind::Decision));
  assert_eq!(row.connection.as_deref(), Some("Decision"));
  assert_eq!(row.model, Some(model()));
  assert_eq!(row.last_test, Some(figures));
  assert_eq!(row.destination.expect("destination").host, "review.example");
  assert_eq!(row.unavailable, None);
  assert!(
    project_settings::review_options(
      &project_settings::load_in(repo.path()).expect("settings"),
      &hosted.id().to_string(),
      "jev-latest"
    )
    .row
    .last_test
    .is_some()
  );
  db.save_auto_mode_settings(&settings(&hosted, "0.90"))
    .expect("threshold");
  assert!(
    reviewer::read(&db).expect("read").last_test.is_none(),
    "never use another threshold's figures"
  );
  hosted.record_check(DecisionCheck {
    at: Timestamp::now(),
    result: DecisionCheckResult::KeyRefused,
  });
  db.save_decision_connection(&hosted).expect("failed check");
  let row = reviewer::read(&db).expect("read");
  assert_eq!(row.unavailable, Some(ReviewerUnavailable::NotConnected));
  assert_eq!(row.last_check, Some(DecisionCheckResult::KeyRefused));
  assert_eq!(row.model, Some(model()));
  assert!(row.destination.is_some());
  hosted.record_check(DecisionCheck {
    at: Timestamp::now(),
    result: DecisionCheckResult::Connected,
  });
  hosted.record_models(Vec::new());
  db.save_decision_connection(&hosted).expect("models gone");
  assert_eq!(
    reviewer::read(&db).expect("read").unavailable,
    Some(ReviewerUnavailable::ModelGone)
  );
  let missing = AutoModeSettings {
    reviewer: Some(ReviewerChoice::decision(
      DecisionConnectionId::generate(),
      model(),
      None,
    )),
  };
  let row = reviewer::row(&missing, &[], &[]);
  assert_eq!(row.unavailable, Some(ReviewerUnavailable::ConnectionGone));
  assert_eq!(row.kind, Some(ReviewerKind::Decision));
  assert_eq!(row.model, Some(model()));
}

#[test]
fn decision_reviewer_setting_refuses_destination_changes_after_validation() {
  use orchester_store::AutoModeSave;

  let (repo, mut db) = project();
  let mut writer = ProjectDb::open(repo.path()).expect("other writer");
  for original_url in ["http://localhost:8000/v1", "https://confirmed.example/v1"] {
    let original = connection(original_url);
    db.create_decision_connection(&original).expect("connection");
    let base = db.auto_mode_settings().expect("baseline");
    let asked = settings(&original, "0.80");
    let validated = db.decision_connections().expect("validated snapshot");
    let mut changed = original.clone();
    changed.set_server(
      DecisionServerUrl::new("https://different.example/v1").expect("url"),
      None,
    );
    changed.record_models(vec![model()]);
    changed.record_check(DecisionCheck {
      at: Timestamp::now(),
      result: DecisionCheckResult::Connected,
    });
    writer
      .save_decision_connection(&changed)
      .expect("concurrent destination edit");
    assert_eq!(
      db.save_auto_mode_with_decision_if_current(&base, &asked, &validated)
        .expect("compare and save"),
      AutoModeSave::ChangedOutside
    );
    assert_eq!(db.auto_mode_settings().expect("unchanged"), base);
    writer
      .save_decision_connection(&original)
      .expect("restore validated destination");
    assert_eq!(
      db.save_auto_mode_with_decision_if_current(&base, &asked, &validated)
        .expect("current destination"),
      AutoModeSave::Saved
    );
  }
}

#[test]
fn decision_reviewer_setting_confirmation_is_bound_to_displayed_url_and_key() {
  let (repo, mut db) = project();
  let original = connection("https://shown.example/v1");
  db.create_decision_connection(&original).expect("connection");
  let base = db.auto_mode_settings().expect("baseline");
  let asked = settings(&original, "0.80");
  for (url, key) in [
    ("https://different.example/v1", None),
    (
      "https://shown.example/v1",
      Some(orchester_core::id::SecretId::generate()),
    ),
  ] {
    let mut changed = original.clone();
    changed.set_server(DecisionServerUrl::new(url).expect("url"), key);
    changed.record_models(vec![model()]);
    changed.record_check(original.last_check().expect("checked connection").clone());
    db.save_decision_connection(&changed).expect("changed destination");
    assert_eq!(
      project_settings::save_auto_mode_in(repo.path(), &base, &asked, true, Some(&original))
        .expect_err("stale displayed connection")
        .key,
      "projectSettings.changedOutside"
    );
    assert_eq!(db.auto_mode_settings().expect("unchanged"), base);
  }
}
