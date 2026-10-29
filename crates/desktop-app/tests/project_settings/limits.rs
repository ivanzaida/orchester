//! Empty limits, validation, saving, status-bar readback and edits made outside the page; raising or removing the
//! daily budget as a person's decision (ORC-348, `docs/design/approval-modes.md`, "Decisions").
use lurq::app::events::with_synthetic_input;
use orchester_core::{
  limits::{AgentSlots, ProjectLimits, QaSlots},
  run::AllowAutopilot,
  store::SettingsStore,
  tokens::TokenBudget,
};
use orchester_workspace::{app_mcp::ToolError, tasks::outside::Outside};
use serde_json::json;

use super::{
  domain::{call, consent},
  harness::Settings,
};

/// The project allows Autopilot, as the person turned it on.
fn autopilot(page: &Settings) {
  page
    .db()
    .save_allow_autopilot(AllowAutopilot::ON)
    .expect("Autopilot allowed");
}

fn daily(page: &Settings) -> Option<u64> {
  page
    .db()
    .project_limits()
    .expect("limits")
    .daily_budget_limit()
    .map(TokenBudget::get)
}

#[test]
fn decisions_reach_person_daily_budget_raise_over_mcp_waits_under_autopilot_until_the_person_lets_automation_answer() {
  let page = Settings::new(false);
  autopilot(&page);
  for asked in [json!(null), json!(30_000_000)] {
    let refused = call(
      &page,
      "update_project_limits",
      json!({ "project": "Kontur", "daily_budget": asked }),
    )
    .expect_err("the person's");
    assert!(
      matches!(&refused, ToolError::Refused { code, .. } if code == "automation_not_allowed"),
      "{asked}: {refused:?}"
    );
  }
  assert_eq!(daily(&page), Some(20_000_000), "nothing saved");
  let lowered = call(
    &page,
    "update_project_limits",
    json!({ "project": "Kontur", "daily_budget": 10_000_000 }),
  )
  .expect("lowering it takes no consent");
  assert_eq!(lowered["limits"]["daily_budget"], 10_000_000);
  consent(&page);
  let lifted = call(
    &page,
    "update_project_limits",
    json!({ "project": "Kontur", "daily_budget": null }),
  )
  .expect("with the person's consent");
  assert!(lifted["limits"]["daily_budget"].is_null(), "{lifted}");
}

#[test]
fn decisions_reach_person_limits_tab_refuses_a_clients_daily_budget_raise_until_the_person_lets_automation_answer() {
  let mut page = Settings::new(false);
  autopilot(&page);
  page.tab("limits", "settings-default-budget");
  page.set("settings-daily-budget", "");
  with_synthetic_input(|| page.leave());
  page.until("the client's removal refused", |p| {
    p.exists("settings-limits-person-only")
  });
  assert_eq!(daily(&page), Some(20_000_000), "nothing saved");
  // Lowering it takes no consent.
  page.set("settings-daily-budget", "10000000");
  with_synthetic_input(|| page.leave());
  page.saved();
  assert_eq!(daily(&page), Some(10_000_000));
  assert!(!page.exists("settings-limits-person-only"));
  // The person's own edit removes it.
  page.set("settings-daily-budget", "");
  page.leave();
  page.saved();
  assert_eq!(daily(&page), None);
  page.set("settings-daily-budget", "10000000");
  page.leave();
  page.saved();
  consent(&page);
  page.set("settings-daily-budget", "40000000");
  with_synthetic_input(|| page.leave());
  page.saved();
  assert_eq!(daily(&page), Some(40_000_000), "with the person's consent");
  assert!(!page.exists("settings-limits-person-only"));
}

#[test]
fn project_settings_limits_show_no_limit_and_save_all_fields_without_a_maximum() {
  let mut page = Settings::new(false);
  page.tab("limits", "settings-default-budget");
  // A limit that is not set leaves its box empty under the No limit placeholder, with nothing read back.
  assert_eq!(page.value("settings-default-budget"), "");
  assert!(!page.exists("settings-default-budget-read-back"));
  assert!(!page.exists("settings-time-limit-read-back"));
  page.set("settings-default-budget", "900000000");
  page.leave();
  page.saved();
  assert_eq!(
    page
      .db()
      .project_limits()
      .expect("limits")
      .default_run_budget()
      .map(TokenBudget::get),
    Some(900_000_000)
  );
  page.set("settings-daily-budget", "");
  page.leave();
  page.saved();
  assert_eq!(page.db().project_limits().expect("limits").daily_budget_limit(), None);
  assert!(!page.exists("settings-daily-budget-read-back"), "its box says No limit");
  page.set("settings-time-limit", "1200");
  page.leave();
  page.saved();
  page.set("settings-agent-slots", "16");
  page.leave();
  page.saved();
  let read = page.db().project_limits().expect("limits");
  assert_eq!(read.default_time_limit().map(|v| v.as_secs()), Some(72000));
  assert_eq!(read.agent_slots().get().get(), 16);
  page.until("the usage card's no-limit readback", |p| {
    p.window
      .texts("sidebar-usage-value")
      .is_some_and(|text| text.join(" ").contains("No limit"))
  });
  page.tab("general", "settings-project-name");
  page.tab("limits", "settings-default-budget");
  assert_eq!(page.value("settings-daily-budget"), "");
  assert_eq!(page.value("settings-agent-slots"), "16");
}

#[test]
fn project_settings_limits_reject_bad_values_in_place_without_saving() {
  let mut page = Settings::new(false);
  page.tab("limits", "settings-default-budget");
  for (id, text) in [
    ("settings-default-budget", "1.4M"),
    ("settings-daily-budget", "0"),
    ("settings-time-limit", "1.5"),
    ("settings-agent-slots", "17"),
  ] {
    page.set(id, text);
    page.leave();
    assert!(
      page.text(&format!("{id}-note")).contains("Still"),
      "the invalid value keeps its saved readback"
    );
    assert_eq!(page.value(id), text, "the person's edit stays");
  }
  assert_eq!(page.db().project_limits().expect("unchanged"), ProjectLimits::DEFAULT);
  assert_eq!(
    page.text("project-settings-save-status"),
    "Not saved · fix the marked field"
  );
}

#[test]
fn project_settings_limits_preserve_outside_fields_and_reload_unsaved_edits() {
  let mut page = Settings::new(false);
  page.tab("limits", "settings-default-budget");
  page.set("settings-default-budget", "5000000");
  let outside = ProjectLimits::DEFAULT.with_agent_slots(AgentSlots::new(2).expect("slots"));
  page.db().save_project_limits(&outside).expect("outside save");
  page.outside(Outside::Limits);
  page.until("outside notice", |p| p.exists("settings-changed-outside"));
  assert_eq!(page.value("settings-default-budget"), "5000000");
  page.leave();
  page.saved();
  assert_eq!(page.db().project_limits().expect("merged").agent_slots().get().get(), 2);
  page.set("settings-default-budget", "bad");
  page
    .db()
    .save_project_limits(&ProjectLimits::DEFAULT)
    .expect("another outside save");
  page.outside(Outside::Limits);
  page.until("outside notice", |p| p.exists("settings-reload"));
  page.click("settings-reload");
  assert_eq!(page.value("settings-default-budget"), "");
  assert_eq!(page.value("settings-agent-slots"), "4");
}

#[test]
fn project_settings_limits_qa_apps_at_once_saves_like_agents_and_rejects_bad_values() {
  let mut page = Settings::new(false);
  page.tab("limits", "settings-default-budget");
  assert_eq!(page.value("settings-qa-slots"), "2");
  assert_eq!(page.text("settings-qa-hint"), "1 to 16");
  assert!(
    !page.exists("settings-qa-slots-read-back"),
    "a count reads nothing back"
  );
  assert!(page.text("settings-qa-row").contains("QA apps at once"));
  page.set("settings-qa-slots", "3");
  page.leave();
  page.saved();
  let read = page.db().project_limits().expect("limits");
  assert_eq!(read.qa_slots().get().get(), 3);
  assert_eq!(read.agent_slots(), AgentSlots::DEFAULT, "the agents' count is its own");
  for text in ["0", "17", "two"] {
    page.set("settings-qa-slots", text);
    page.leave();
    let note = page.text("settings-qa-slots-note");
    assert!(
      note.contains("Enter a whole number from 1 to 16.") && note.contains("Still 3."),
      "{text}: {note}"
    );
    assert_eq!(page.value("settings-qa-slots"), text, "the person's edit stays");
  }
  assert_eq!(page.db().project_limits().expect("unchanged").qa_slots().get().get(), 3);
  page.set("settings-qa-slots", "3");
  page.leave();
  page.saved();
  // A change made outside the page to another limit keeps the QA apps, and the page shows it.
  let outside = page
    .db()
    .project_limits()
    .expect("limits")
    .with_agent_slots(AgentSlots::new(6).expect("slots"));
  page.db().save_project_limits(&outside).expect("outside save");
  page.outside(Outside::Limits);
  page.until("the outside change shown", |p| p.value("settings-agent-slots") == "6");
  assert_eq!(page.value("settings-qa-slots"), "3");
  let stored = ProjectLimits::DEFAULT.with_qa_slots(QaSlots::new(16).expect("slots"));
  page.db().save_project_limits(&stored).expect("outside save");
  page.outside(Outside::Limits);
  page.until("the outside QA apps shown", |p| p.value("settings-qa-slots") == "16");
}

#[test]
fn project_settings_limits_failed_edit_reverts_all_fields_without_a_write_and_unlocks_navigation() {
  let mut page = Settings::new(false);
  page.tab("limits", "settings-default-budget");
  let original = page.db().project_limits().expect("baseline");
  let fields = [
    ("settings-default-budget", "2000000"),
    ("settings-daily-budget", "5000000"),
    ("settings-time-limit", "60"),
    ("settings-agent-slots", "8"),
  ];
  let baseline: Vec<_> = fields.iter().map(|(id, _)| (*id, page.value(id))).collect();
  let sql = rusqlite::Connection::open(page.repo.join(".orchester/orchester.db")).expect("fixture DB");
  sql
    .execute_batch(
      "CREATE TRIGGER reject_limits BEFORE INSERT ON project_limits
     BEGIN SELECT RAISE(ABORT, 'regression write refused'); END;",
    )
    .expect("controlled failure");
  for (id, draft) in fields {
    page.set(id, draft);
  }
  page.leave();
  page.until("limits save failed", |p| p.exists("settings-limits-retry"));
  assert_eq!(page.db().project_limits().expect("unchanged"), original);
  page.click("settings-tab-general");
  assert!(page.exists("settings-limits-retry"));
  // The rejection remains installed: Saved must come from abandonment, not an extra write.
  for (id, value) in baseline {
    page.set(id, &value);
  }
  page.leave();
  page.saved();
  assert!(!page.exists("settings-limits-retry"));
  assert!(!page.exists("settings-save-error"));
  assert_eq!(page.db().project_limits().expect("baseline kept"), original);
  page.tab("general", "settings-project-name");
  page.tab("limits", "settings-default-budget");
  assert_eq!(page.db().project_limits().expect("no later write"), original);
}

#[test]
fn project_settings_limits_same_value_recovery_does_not_submit_an_extra_write() {
  let mut page = Settings::new(false);
  page.tab("limits", "settings-default-budget");
  let sql = rusqlite::Connection::open(page.repo.join(".orchester/orchester.db")).expect("fixture DB");
  sql
    .execute_batch(
      "CREATE TRIGGER reject_limits BEFORE INSERT ON project_limits
     BEGIN SELECT RAISE(ABORT, 'regression write refused'); END;",
    )
    .expect("controlled failure");
  page.set("settings-default-budget", "2000000");
  page.leave();
  page.until("limits failure", |p| p.exists("settings-limits-retry"));
  // Focus the failed field while its draft is still rejected; then measure only baseline recovery.
  page.set("settings-default-budget", "2000000");
  page.until("focused failed draft", |p| {
    p.text("project-settings-save-status").starts_with("Not saved") && p.exists("settings-limits-retry")
  });
  // Count subsequent writes instead of refusing them, so an unnecessary same-value save cannot hide.
  sql
    .execute_batch(
      "DROP TRIGGER reject_limits;
     CREATE TABLE recovery_writes (id INTEGER);
     CREATE TRIGGER count_limits BEFORE INSERT ON project_limits
     BEGIN INSERT INTO recovery_writes VALUES (1); END;",
    )
    .expect("write counter");
  page.set("settings-default-budget", "");
  page.leave();
  page.saved();
  page.tab("general", "settings-project-name");
  let writes: i64 = sql
    .query_row("SELECT COUNT(*) FROM recovery_writes", [], |r| r.get(0))
    .expect("count");
  assert_eq!(writes, 0, "abandonment must not save the already-stored limits");
}
