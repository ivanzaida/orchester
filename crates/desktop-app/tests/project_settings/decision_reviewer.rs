//! Decision reviewers in the Runs row, including the person-only destination question.
use lurq::app::events::with_synthetic_input;
use orchester_core::{
  connection::{
    DecisionCheck, DecisionCheckResult, DecisionConnection, DecisionServerUrl, DecisionTest, DecisionThresholdFigures,
  },
  id::DecisionConnectionId,
  name::Name,
  resource::ModelId,
  time::Timestamp,
};
use orchester_workspace::{project_settings::decision_reviewer::threshold, tasks::outside::Outside};

use super::harness::Settings;

fn add(page: &mut Settings, hosted: bool) -> DecisionConnection {
  let mut connection = DecisionConnection::new(
    DecisionConnectionId::generate(),
    Name::new(if hosted { "Hosted reviewer" } else { "Local reviewer" }).expect("name"),
    DecisionServerUrl::new(if hosted {
      "https://review.example/v1"
    } else {
      "http://127.0.0.1:8090/v1"
    })
    .expect("url"),
    None,
  )
  .with_models(vec![ModelId::new("decision-model").expect("model")]);
  connection.record_check(DecisionCheck {
    at: Timestamp::now(),
    result: DecisionCheckResult::Connected,
  });
  connection.record_test(DecisionTest {
    at: Timestamp::now(),
    model: connection.models()[0].clone(),
    figures: vec![DecisionThresholdFigures {
      threshold: threshold("0.90").expect("threshold"),
      approves_on_its_own: 8141,
      comes_to_person: 1662,
      approves_earlier_reviewer_asks: 0,
      requests_tested: 9803,
      median_ms: 12,
      tokens: 500,
    }],
  });
  page.db().create_decision_connection(&connection).expect("connection");
  page.outside(Outside::Connections);
  page.until("connections reread", |p| {
    p.text("project-settings-save-status") == "All changes saved"
  });
  // Remount on the current database rather than racing the connection refresh.
  page.tab("general", "settings-project-name");
  page.tab("runs", "settings-reviewer-connection");
  connection
}

#[test]
fn decision_reviewer_row_local_threshold_figures_and_unavailable() {
  let mut page = Settings::new(true);
  let mut local = add(&mut page, false);
  page.choose("settings-reviewer-connection", "Local reviewer");
  page.until("local reviewer saved", |p| p.reviewer_model() == "decision-model");
  assert!(!page.exists("settings-reviewer-effort"));
  assert!(page.exists("settings-reviewer-threshold"));
  assert!(
    page
      .text("settings-reviewer-destination")
      .contains("Requests stay on this computer")
  );
  page.set("settings-reviewer-threshold", "0.90");
  page.leave();
  page.until("threshold saved", |p| {
    p.db()
      .auto_mode_settings()
      .expect("settings")
      .reviewer
      .expect("reviewer")
      .threshold
      == Some(threshold("0.90").expect("threshold"))
  });
  assert!(
    page
      .text("settings-reviewer-destination")
      .contains("approved 83% on its own at 0.90")
  );
  page.set("settings-reviewer-threshold", "1.00");
  page.leave();
  assert!(page.exists("settings-reviewer-threshold-note"));
  assert_eq!(
    page
      .db()
      .auto_mode_settings()
      .expect("settings")
      .reviewer
      .expect("reviewer")
      .threshold,
    Some(threshold("0.90").expect("threshold"))
  );
  page.set("settings-reviewer-threshold", "0.9");
  page.leave();
  page.saved();
  assert_eq!(page.value("settings-reviewer-threshold"), "0.90");
  local.record_check(DecisionCheck {
    at: Timestamp::now(),
    result: DecisionCheckResult::NothingAnswers,
  });
  page
    .db()
    .save_decision_connection(&local)
    .expect("connection unavailable");
  page.outside(Outside::Connections);
  page.tab("general", "settings-project-name");
  page.tab("runs", "settings-reviewer-threshold");
  assert!(
    page
      .text("settings-reviewer-destination")
      .contains("Nothing answers at 127.0.0.1:8090")
  );
  assert!(page.exists("settings-reviewer-check"));
}

#[test]
fn decision_reviewer_row_hosted_question_refuses_automation_and_keeps_the_previous_choice() {
  let mut page = Settings::new(true);
  add(&mut page, true);
  let before = page.db().auto_mode_settings().expect("settings");
  page.choose("settings-reviewer-connection", "Hosted reviewer");
  page.until("destination question", |p| p.exists("settings-reviewer-hosted-confirm"));
  assert!(page.text("settings-reviewer-hosted").contains("review.example"));
  assert!(page.window.link.input_gate().protecting());
  with_synthetic_input(|| page.click("settings-reviewer-hosted-confirm"));
  assert_eq!(page.db().auto_mode_settings().expect("unchanged"), before);
  page.click("settings-reviewer-hosted-keep");
  assert!(!page.exists("settings-reviewer-hosted-confirm"));
  assert!(!page.window.link.input_gate().protecting());
  assert_eq!(page.db().auto_mode_settings().expect("kept"), before);
  page.choose("settings-reviewer-connection", "Hosted reviewer");
  page.until("destination question armed", |p| {
    p.exists("settings-reviewer-hosted-confirm")
  });
  page.window.tree.tick_futures();
  page.window.pass();
  page.click("settings-reviewer-hosted-confirm");
  page.until("person confirmed", |p| p.reviewer_model() == "decision-model");
  assert!(!page.window.link.input_gate().protecting());
  page.saved();
  page.choose("settings-reviewer-connection", "Claude Code");
  page.until("language reviewer saved", |p| p.reviewer_model() != "decision-model");
  page.saved();
  page.choose("settings-reviewer-connection", "Hosted reviewer");
  assert!(
    page.exists("settings-reviewer-hosted-confirm"),
    "earlier consent cannot answer a new choice"
  );
}

#[test]
fn decision_reviewer_row_local_automation_requires_both_consents() {
  for (approvals, permissions) in [(false, false), (true, false), (false, true), (true, true)] {
    let mut page = Settings::new(true);
    add(&mut page, false);
    let workspace = page.window.launch.workspace().expect("workspace").clone();
    workspace
      .set_automation_answers_approvals(approvals)
      .expect("approvals consent");
    workspace
      .set_automation_changes_permissions(permissions)
      .expect("permission consent");
    let before = page.db().auto_mode_settings().expect("baseline");
    with_synthetic_input(|| page.choose("settings-reviewer-connection", "Local reviewer"));
    if approvals && permissions {
      page.until("authorized local reviewer", |p| p.reviewer_model() == "decision-model");
    } else {
      page.until("unauthorized reviewer refused", |p| {
        p.exists("settings-auto-person-only")
      });
      assert_eq!(page.db().auto_mode_settings().expect("unchanged"), before);
    }
  }
}

#[test]
fn decision_reviewer_row_hosted_confirmation_rejects_unseen_destination() {
  let mut page = Settings::new(true);
  let mut hosted = add(&mut page, true);
  let before = page.db().auto_mode_settings().expect("baseline");
  page.choose("settings-reviewer-connection", "Hosted reviewer");
  page.until("destination question", |p| p.exists("settings-reviewer-hosted-confirm"));
  hosted.set_server(DecisionServerUrl::new("https://other.example/v1").expect("url"), None);
  hosted.record_models(vec![ModelId::new("decision-model").expect("model")]);
  hosted.record_check(DecisionCheck {
    at: Timestamp::now(),
    result: DecisionCheckResult::Connected,
  });
  page
    .db()
    .save_decision_connection(&hosted)
    .expect("unseen destination change");
  page.click("settings-reviewer-hosted-confirm");
  page.until("stale confirmation refused", |p| p.exists("settings-save-error"));
  assert_eq!(page.db().auto_mode_settings().expect("unchanged"), before);
}

#[test]
fn decision_reviewer_row_failed_reread_removes_hosted_confirmation_until_retry() {
  let mut page = Settings::new(true);
  add(&mut page, true);
  let before = page.db().auto_mode_settings().expect("baseline");
  page.choose("settings-reviewer-connection", "Hosted reviewer");
  page.until("destination question", |p| p.exists("settings-reviewer-hosted-confirm"));
  let sqlite = rusqlite::Connection::open(page.repo.join(".orchester/orchester.db")).expect("db");
  let version: u32 = sqlite
    .query_row("PRAGMA user_version", [], |r| r.get(0))
    .expect("schema");
  sqlite
    .execute_batch("PRAGMA user_version = 99999")
    .expect("reject reads");
  page.outside(Outside::Connections);
  page.window.until("settings read rejected", |w| {
    w.tree.get_element_by_id("settings-load-error").is_some()
  });
  assert!(!page.exists("settings-reviewer-hosted-confirm"));
  sqlite
    .execute_batch(&format!("PRAGMA user_version = {version}"))
    .expect("restore schema");
  assert!(
    !page.exists("settings-reviewer-hosted-confirm"),
    "only a successful read can restore controls"
  );
  page.click("settings-load-error-retry");
  page.until("fresh settings read", |p| p.exists("settings-reviewer-connection"));
  assert_eq!(page.db().auto_mode_settings().expect("unchanged"), before);
  assert!(
    !page.exists("settings-reviewer-hosted-confirm"),
    "discard old consent question"
  );
}

#[test]
fn decision_reviewer_row_threshold_and_return_require_approvals_but_not_permissions() {
  let mut page = Settings::new(true);
  add(&mut page, false);
  page.choose("settings-reviewer-connection", "Local reviewer");
  page.until("local reviewer saved", |p| p.reviewer_model() == "decision-model");
  page.saved();
  let workspace = page.window.launch.workspace().expect("workspace").clone();
  workspace
    .set_automation_changes_permissions(false)
    .expect("permissions off");
  workspace.set_automation_answers_approvals(true).expect("approvals on");
  with_synthetic_input(|| {
    page.set("settings-reviewer-threshold", "0.90");
    page.leave();
  });
  page.until("threshold saved with approvals consent", |p| {
    p.db()
      .auto_mode_settings()
      .expect("settings")
      .reviewer
      .expect("reviewer")
      .threshold
      == Some(threshold("0.90").expect("threshold"))
  });
  page.saved();
  workspace
    .set_automation_answers_approvals(false)
    .expect("approvals off");
  with_synthetic_input(|| page.choose("settings-reviewer-connection", "Claude Code"));
  page.until("return without approvals refused", |p| {
    p.exists("settings-auto-person-only")
  });
  assert_eq!(page.reviewer_model(), "decision-model");
  workspace.set_automation_answers_approvals(true).expect("approvals on");
  with_synthetic_input(|| page.choose("settings-reviewer-connection", "Claude Code"));
  page.until("return to language with approvals consent", |p| {
    p.reviewer_model() != "decision-model"
  });
}
