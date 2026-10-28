//! Existing run authority is independent of the future-run default.
use orchester_workspace::{
  project_settings,
  tasks::{outside::Outside, run_approvals},
};
use serde_json::json;

use super::{domain, harness::Settings, run_fixture};

#[test]
fn project_settings_reviewer_change_for_existing_auto_run_is_a_persons() {
  let mut page = Settings::new(true);
  page.choose("settings-reviewer-connection", "Claude Code");
  page.saved();
  let original = page.db().auto_mode_settings().expect("reviewer A");
  assert!(!original.new_runs);
  let run = run_fixture::auto_run(&mut page.db());
  page.outside(Outside::Runs);
  // Reload from the run feed before editing, without replacing this window.
  page.until("existing Auto run read", |p| {
    project_settings::load_in(&p.repo).expect("settings").auto_runs
  });
  page.tab("general", "settings-project-name");
  page.tab("runs", "settings-reviewer-model");
  assert!(matches!(
    domain::call(&page, "set_auto_mode", json!({"project":"Kontur",
      "reviewer":{"connection":"Claude Code","model":"sonnet"}})).expect_err("automation OFF"),
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
  assert!(!page.db().auto_mode_settings().expect("future default").new_runs);
  let resolved = run_approvals::auto_mode_in(&page.repo, run).expect("active run");
  assert!(resolved.on);
  assert_eq!(resolved.reviewer.expect("reviewer B").model.as_str(), "sonnet");
}

#[test]
fn project_settings_auto_save_rechecks_existing_runs_and_keeps_narrow_cases() {
  let page = Settings::new(true);
  let mut db = page.db();
  let base = db.auto_mode_settings().expect("base");
  let settings = project_settings::load_in(&page.repo).expect("old page read");
  let mut asked = base.clone();
  asked.reviewer = project_settings::reviewer(&settings, "", "sonnet", "").expect("B");
  let run = run_fixture::auto_run(&mut db);
  assert_eq!(
    project_settings::save_auto_mode_in(&page.repo, &base, &asked, false)
      .expect_err("run became Auto ON after page read")
      .key,
    "projectSettings.personOnly"
  );
  assert_eq!(db.auto_mode_settings().expect("unchanged"), base);
  assert_eq!(
    db.save_auto_mode_if_current(&base, &asked, |auto_runs| !auto_runs)
      .expect("atomic authority check"),
    orchester_store::AutoModeSave::Refused
  );
  project_settings::save_auto_mode_in(&page.repo, &base, &base, false).expect("unchanged reviewer is narrow");
  db.set_run_auto_mode(run, false).expect("run Off");
  project_settings::save_auto_mode_in(&page.repo, &base, &asked, false).expect("no active reviewer");
  assert_eq!(db.auto_mode_settings().expect("saved B"), asked);
}
