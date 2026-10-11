//! General's stored identity, agent-file switch and reviewer/auto-mode actions.
use orchester_core::store::SettingsStore;

use super::harness::Settings;

#[test]
fn project_settings_general_shows_project_identity_and_saves_repository_agent_files() {
  let mut page = Settings::new(false);
  assert_eq!(page.value("settings-project-name"), "Kontur");
  assert_eq!(page.value("settings-project-key"), "KON");
  assert_eq!(page.value("settings-project-folder"), page.repo.display().to_string());
  assert!(page.text("settings-name-row").contains("Renaming is not available yet"));
  assert!(page.exists("settings-github"));
  let before = page.db().repository_agent_files().expect("setting").is_on();
  page.click("settings-agent-files");
  page.until("agent-file save", |p| {
    p.db().repository_agent_files().expect("saved").is_on() != before
  });
  page.saved();
  assert_eq!(page.db().repository_agent_files().expect("saved").is_on(), !before);
  assert!(page.text("settings-files-claude-values").contains("none"));
  assert!(page.text("settings-import-reason").contains("not built"));
}

#[test]
fn project_settings_auto_mode_without_a_reviewer_says_why_it_cannot_turn_on() {
  let mut page = Settings::new(false);
  assert!(page.exists("settings-auto-unavailable"));
  assert!(!page.exists("settings-reviewer-connection"));
  assert!(!page.exists("settings-reviewer-model"));
  assert!(!page.exists("settings-reviewer-effort"));
  assert!(page.text("settings-auto-open-connections").contains("Open Connections"));
  page.click("settings-auto-new-runs");
  assert!(!page.db().auto_mode_settings().expect("auto mode").new_runs);
  assert!(!page.exists("settings-auto-question"));
  page.click("settings-auto-open-connections");
  assert!(matches!(
    page.window.navigation().route_now(),
    orchester_app::routes::Route::Project(_, orchester_app::routes::ProjectPage::Connections(_))
  ));
}

#[test]
fn project_settings_auto_mode_asks_a_person_then_saves_and_can_turn_off_without_asking() {
  let mut page = Settings::new(true);
  assert!(page.text("settings-reviewer-readback").to_lowercase().contains("haiku"));
  page.click("settings-auto-new-runs");
  page.until("auto-mode question", |p| p.exists("settings-auto-question-confirm"));
  assert!(page.window.link.input_gate().protecting());
  assert!(!page.db().auto_mode_settings().expect("unchanged").new_runs);
  page.window.tree.tick_futures();
  page.window.pass();
  page.click("settings-auto-question-confirm");
  page.until("auto mode saved", |p| {
    p.db().auto_mode_settings().expect("saved").new_runs
  });
  page.saved();
  assert!(!page.window.link.input_gate().protecting());
  page.click("settings-auto-new-runs");
  page.until("auto mode off", |p| {
    !p.db().auto_mode_settings().expect("saved").new_runs
  });
  page.saved();
  assert!(!page.exists("settings-auto-question"));
  page.choose("settings-reviewer-connection", "Claude Code");
  page.until("reviewer saved", |p| {
    p.db().auto_mode_settings().expect("saved").reviewer.is_some()
  });
  page.saved();
  let choice = page
    .db()
    .auto_mode_settings()
    .expect("auto mode")
    .reviewer
    .expect("chosen");
  assert!(!choice.model.as_str().is_empty());
  // Start another selection while General may still be reading back the previous save.
  for model in ["sonnet", "haiku", "sonnet"] {
    page.choose("settings-reviewer-model", model);
    page.until("model saved", |p| {
      p.db()
        .auto_mode_settings()
        .expect("saved")
        .reviewer
        .is_some_and(|choice| choice.model.as_str() == model)
    });
    page.saved();
  }
}

#[test]
fn project_settings_github_check_reads_origin_and_says_when_no_github_remote_exists() {
  let mut page = Settings::new(false);
  super::index::git(&page.repo, &["init"]);
  page.until("repository read", |p| p.exists("settings-github-check"));
  page.click("settings-github-check");
  page.until("origin checked", |p| p.exists("settings-github-detection"));
  assert!(page.text("settings-github-detection").contains("No GitHub remote"));
  assert_eq!(page.value("settings-github-repository"), "Not linked");
  assert!(!page.exists("settings-use-account"));
}
