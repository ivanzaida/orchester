//! The reviewer approves requests in every Manual run in the person's place, so choosing another one is a person's.
use orchester_core::run::{EffectiveMode, ManualBecause};
use orchester_workspace::{project_settings, tasks::run_approvals};
use serde_json::json;

use super::{domain, harness::Settings, run_fixture};

#[test]
fn project_settings_reviewer_change_for_a_manual_run_is_a_persons() {
  let mut page = Settings::new(true);
  page.choose("settings-reviewer-connection", "Claude Code");
  page.saved();
  let original = page.db().auto_mode_settings().expect("reviewer A");
  let run = run_fixture::started_run(&mut page.db());
  assert!(matches!(
    domain::call(&page, "set_reviewer", json!({"project":"Kontur",
      "connection":"Claude Code","model":"sonnet"})).expect_err("automation OFF"),
    orchester_workspace::app_mcp::ToolError::Refused { code, .. } if code == "automation_not_allowed"
  ));
  // The window's own input from automation is put back too.
  lurq::app::events::with_synthetic_input(|| page.choose("settings-reviewer-model", "sonnet"));
  page.until("the reviewer put back", |p| p.exists("settings-auto-person-only"));
  assert_eq!(page.db().auto_mode_settings().expect("unchanged"), original);
  page.choose("settings-reviewer-model", "sonnet");
  page.until("a person's reviewer saved", |p| p.reviewer_model() == "sonnet");
  page.saved();
  assert!(!page.window.link.input_gate().protecting());
  let resolved = run_approvals::run_mode_in(&page.repo, run).expect("active run");
  assert_eq!(resolved.mode, EffectiveMode::Manual(ManualBecause::Profile));
  assert_eq!(resolved.reviewer.expect("reviewer B").model.as_str(), "sonnet");
}

#[test]
fn project_settings_auto_save_needs_a_person_for_another_reviewer_and_keeps_narrow_cases() {
  let page = Settings::new(true);
  let db = page.db();
  let base = db.auto_mode_settings().expect("base");
  let settings = project_settings::load_in(&page.repo).expect("old page read");
  let mut asked = base.clone();
  asked.reviewer = project_settings::reviewer(&settings, "", "sonnet", "").expect("B");
  assert_eq!(
    project_settings::save_auto_mode_in(&page.repo, &base, &asked, false)
      .expect_err("another reviewer from input that may not choose it")
      .key,
    "projectSettings.personOnly"
  );
  assert_eq!(db.auto_mode_settings().expect("unchanged"), base);
  project_settings::save_auto_mode_in(&page.repo, &base, &base, false).expect("unchanged reviewer is narrow");
  project_settings::save_auto_mode_in(&page.repo, &base, &asked, true).expect("a person's choice");
  assert_eq!(db.auto_mode_settings().expect("saved B"), asked);
}
