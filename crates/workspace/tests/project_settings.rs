//! Project settings' validation and compare-and-save rules, independently of the components.
use std::{fs, time::Duration};

use orchester_core::{
  id::ProjectId,
  limits::{ProjectLimits, QaSlots},
  name::Name,
  project::{Project, ProjectKey},
  run::review::AutoModeSettings,
  store::SettingsStore,
  tokens::TokenBudget,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  environment,
  project_settings::{self, LimitsText},
  tools::form::parse_environment,
};

fn project() -> (tempfile::TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary project");
  fs::create_dir(repo.path().join(".git")).expect("git folder");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("key"),
    Name::new("Settings").expect("name"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("project database");
  (repo, db)
}
#[test]
fn project_settings_limits_are_optional_without_a_product_maximum_and_report_each_error() {
  let text = LimitsText {
    slots: "4".into(),
    qa: "2".into(),
    ..LimitsText::default()
  };
  let limits = text.limits(ProjectLimits::DEFAULT, ",").expect("no limits");
  assert_eq!(
    (
      limits.default_run_budget(),
      limits.daily_budget_limit(),
      limits.default_time_limit()
    ),
    (None, None, None)
  );
  let large = LimitsText {
    default: "900,000,000".into(),
    daily: "1".into(),
    minutes: "400000".into(),
    slots: "16".into(),
    qa: "16".into(),
  };
  let read = large.limits(limits, ",").expect("large limits");
  assert_eq!(read.default_run_budget().map(TokenBudget::get), Some(900_000_000));
  assert_eq!(read.default_time_limit(), Some(Duration::from_secs(400000 * 60)));
  assert_eq!(read.qa_slots().get().get(), 16);
  let invalid = LimitsText {
    default: "one".into(),
    daily: "0".into(),
    minutes: "1.5".into(),
    slots: "17".into(),
    qa: "0".into(),
  };
  let errors = invalid.limits(limits, ",").expect_err("invalid");
  assert!(
    errors.default_run_budget.is_some()
      && errors.daily_budget.is_some()
      && errors.default_time_limit.is_some()
      && errors.agent_slots.is_some()
      && errors.qa_slots.is_some()
  );
}

#[test]
fn project_settings_qa_apps_at_once_read_back_and_refuse_outside_one_to_sixteen() {
  let stored = ProjectLimits::DEFAULT.with_qa_slots(QaSlots::new(3).expect("in range"));
  let text = LimitsText::of(stored, ",");
  assert_eq!((text.slots.as_str(), text.qa.as_str()), ("4", "3"));
  assert_eq!(text.limits(stored, ","), Ok(stored), "read back as stored");
  let raised = LimitsText {
    qa: " 7 ".into(),
    ..text.clone()
  };
  let read = raised.limits(stored, ",").expect("seven");
  assert_eq!(read.qa_slots().get().get(), 7);
  assert_eq!(read.agent_slots(), stored.agent_slots(), "the agents' count is its own");
  for qa in ["", "0", "17", "2.5", "two"] {
    let errors = LimitsText {
      qa: qa.into(),
      ..text.clone()
    }
    .limits(stored, ",")
    .expect_err("outside 1 to 16");
    let error = errors.qa_slots.expect("the QA field's error");
    assert_eq!(error.key, "settings.limits.qaSlotsInvalid", "{qa}");
    assert_eq!(error.args, [("min", "1".to_owned()), ("max", "16".to_owned())]);
    assert_eq!(errors.agent_slots, None, "{qa}");
  }
}
#[test]
fn project_settings_environment_requires_confirmation_and_refuses_a_stale_baseline() {
  let (repo, mut db) = project();
  let base = db.project_environment().expect("environment");
  let harmless = parse_environment("RUST_LOG=info").expect("variables");
  environment::save_checked_in(repo.path(), &base, &harmless, false).expect("harmless save");
  let wide = parse_environment("PATH=tools\nRUST_LOG=debug").expect("variables");
  assert_eq!(
    environment::save_checked_in(repo.path(), &harmless, &wide, false)
      .expect_err("needs person")
      .key,
    "projectSettings.personOnly"
  );
  assert_eq!(db.project_environment().expect("unchanged"), harmless);
  let outside = parse_environment("CI=1").expect("variables");
  db.save_project_environment(&outside).expect("outside writer");
  assert_eq!(
    environment::save_checked_in(repo.path(), &harmless, &wide, true)
      .expect_err("stale")
      .key,
    "projectSettings.changedOutside"
  );
  assert_eq!(db.project_environment().expect("kept outside"), outside);
  environment::save_checked_in(repo.path(), &outside, &wide, true).expect("reviewed current baseline");
  assert_eq!(db.project_environment().expect("saved"), wide);
}
#[test]
fn project_settings_auto_mode_needs_a_reviewer_and_refuses_stale_changes() {
  let (repo, mut db) = project();
  let base = AutoModeSettings::default();
  let asked = AutoModeSettings {
    new_runs: true,
    ..base.clone()
  };
  assert_eq!(
    project_settings::save_auto_mode_in(repo.path(), &base, &asked, false)
      .expect_err("person")
      .key,
    "projectSettings.personOnly"
  );
  assert!(
    project_settings::save_auto_mode_in(repo.path(), &base, &asked, true).is_err(),
    "no reviewer"
  );
  assert_eq!(db.auto_mode_settings().expect("unchanged"), base);
  // A narrow change still cannot overwrite a baseline that changed outside the page.
  db.save_auto_mode_settings(&asked).expect("outside writer");
  assert_eq!(
    project_settings::save_auto_mode_in(repo.path(), &base, &base, false)
      .expect_err("stale")
      .key,
    "projectSettings.changedOutside"
  );
  assert_eq!(db.auto_mode_settings().expect("kept"), asked);
}

#[test]
fn project_settings_default_reviewer_keeps_model_and_effort_edits() {
  use orchester_core::{
    connection::{
      AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
    },
    id::ConnectionId,
    resource::ModelId,
    store::ConnectionStore,
  };
  let (repo, mut db) = project();
  let mut agent = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Codex").expect("name"),
    AgentProvider::CodexCli,
    std::env::current_exe().expect("program"),
    orchester_core::time::Timestamp::now(),
  )
  .expect("agent");
  agent
    .record_check(ConnectionCheck {
      at: orchester_core::time::Timestamp::now(),
      result: CheckResult::Available {
        version: "0.139.0".into(),
        sign_in: SignIn::SignedIn,
        catalog: ModelCatalog::new(
          CatalogSource::Provider,
          vec![CatalogModel::new(
            ModelId::new("gpt-5.5").expect("model"),
            vec!["low".into(), "high".into()],
          )],
        ),
      },
    })
    .expect("check");
  let id = agent.id();
  db.create_connection(&agent.into_connection()).expect("connection");
  let settings = project_settings::load_in(repo.path()).expect("settings");
  assert_eq!(
    project_settings::reviewer(&settings, "", "gpt-5.5", "").expect("default"),
    None
  );
  let choice = project_settings::reviewer(&settings, "", "gpt-5.5", "high")
    .expect("chosen effort")
    .expect("explicit reviewer");
  assert_eq!(choice.connection, id);
  assert_eq!(choice.effort.as_deref(), Some("high"));
  let choice = project_settings::reviewer(&settings, "", "another-model", "")
    .expect("chosen model")
    .expect("explicit reviewer");
  assert_eq!(choice.connection, id);
  assert_eq!(choice.model.as_str(), "another-model");
  assert!(project_settings::reviewer(&settings, "", "gpt-5.5", "invalid").is_err());
}
