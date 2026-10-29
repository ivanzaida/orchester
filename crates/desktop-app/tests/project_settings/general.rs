//! General's identity and repository settings, and Runs' auto mode with its reviewer and *Approved for every run*
//! (`12` `wQy6o`), a page under Runs with Revoke.
use orchester_core::store::SettingsStore;

use super::harness::Settings;

#[test]
fn project_settings_general_shows_project_identity_and_saves_repository_agent_files() {
  let mut page = Settings::new(false);
  page.tab("general", "settings-project-name");
  assert_eq!(page.text("settings-project-name"), "Kontur");
  assert_eq!(page.text("settings-project-key"), "KON");
  assert_eq!(page.text("settings-project-folder"), page.repo.display().to_string());
  assert!(
    !page.text("settings-name-row").contains("not available yet"),
    "a plain value, without the not-available sentence"
  );
  assert!(page.exists("settings-repository"));
  let before = page.db().repository_agent_files().expect("setting").is_on();
  page.click("settings-agent-files");
  page.until("agent-file save", |p| {
    p.db().repository_agent_files().expect("saved").is_on() != before
  });
  page.saved();
  assert_eq!(page.db().repository_agent_files().expect("saved").is_on(), !before);
  page.click("settings-files-loads");
  page.until("What each CLI loads", |p| p.exists("settings-files-panel"));
  assert!(page.text("settings-files-claude-values").contains("none"));
  assert!(page.text("settings-import-reason").contains("not built"));
}

#[test]
fn project_settings_general_turns_off_telling_working_agents_when_the_base_branch_moves() {
  let mut page = Settings::new(false);
  page.tab("general", "settings-base-moved-row");
  assert!(page.db().tells_base_moves().expect("setting"), "on by default");
  assert!(
    page
      .text("settings-base-moved-row")
      .contains("When the base branch moves")
  );
  page.click("settings-base-notices");
  page.until("base notices save", |p| !p.db().tells_base_moves().expect("saved"));
  page.saved();
  assert!(page.text("settings-base-moved-hint").contains("not told"));
}

#[test]
fn project_settings_auto_mode_without_a_reviewer_says_why() {
  let mut page = Settings::new(false);
  assert!(
    page
      .text("settings-auto-reviewer-hint")
      .contains("needs an agent connection")
  );
  assert!(!page.exists("settings-reviewer-connection"));
  assert!(!page.exists("settings-reviewer-model"));
  assert!(!page.exists("settings-reviewer-effort"));
  assert!(page.text("settings-auto-open-connections").contains("Open Connections"));
  page.click("settings-auto-open-connections");
  assert!(matches!(
    page.window.navigation().route_now(),
    orchester_app::routes::Route::Project(_, orchester_app::routes::ProjectPage::Connections(_))
  ));
}

/// Automation's input cannot widen what is approved in the person's place: its allowing Autopilot is refused and its
/// change of the reviewer is put back, each said under its place, and nothing is stored; a person's change then saves.
#[test]
fn project_settings_approval_mode_effective_widening_from_automation_is_put_back() {
  use lurq::app::events::with_synthetic_input;
  let mut page = Settings::new(true);
  with_synthetic_input(|| page.click("settings-allow-autopilot"));
  page.until("the refusal", |p| p.exists("settings-allow-autopilot-person-only"));
  assert!(!page.db().allow_autopilot().expect("unchanged").is_on());
  assert!(!page.switch_on("settings-allow-autopilot"));
  page.click("settings-allow-autopilot");
  page.until("a person's switch saved", |p| {
    p.db().allow_autopilot().expect("saved").is_on()
  });
  page.saved();
  assert!(!page.exists("settings-allow-autopilot-person-only"));
  let before = page.db().auto_mode_settings().expect("the reviewer");
  with_synthetic_input(|| page.choose("settings-reviewer-model", "sonnet"));
  page.until("the reviewer put back", |p| p.exists("settings-auto-person-only"));
  for _ in 0..10 {
    page.window.tree.tick_futures();
    page.window.pass();
  }
  assert_eq!(page.db().auto_mode_settings().expect("kept"), before);
  page.choose("settings-reviewer-model", "sonnet");
  page.until("a person's reviewer saved", |p| {
    p.db()
      .auto_mode_settings()
      .expect("saved")
      .reviewer
      .is_some_and(|choice| choice.model.as_str() == "sonnet")
  });
  page.saved();
}

/// A person's switch saves at once; nothing is asked and no input is locked.
#[test]
fn project_settings_approval_mode_effective_saves_a_persons_switch_at_once_and_turns_off() {
  let mut page = Settings::new(true);
  page.click("settings-allow-autopilot");
  assert_eq!(page.text("project-settings-save-status"), "Saving…");
  assert!(!page.exists("settings-auto-question"), "nothing is asked");
  assert!(!page.window.link.input_gate().protecting());
  page.until("Autopilot allowed", |p| {
    p.db().allow_autopilot().expect("saved").is_on()
  });
  page.saved();
  assert!(page.switch_on("settings-allow-autopilot"));
  page.click("settings-allow-autopilot");
  page.until("Autopilot not allowed", |p| {
    !p.db().allow_autopilot().expect("saved").is_on()
  });
  page.saved();
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
  // Start another selection while Runs may still be reading back the previous save. A model is named by its id.
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
  page.tab("general", "settings-project-name");
  super::index::git(&page.repo, &["init"]);
  page.until("repository read", |p| p.exists("settings-github-check"));
  page.click("settings-github-check");
  page.until("origin checked", |p| {
    p.exists("settings-github-repo-hint") && p.text("settings-github-repo-hint").contains("No GitHub remote")
  });
  assert!(!page.exists("settings-use-account"));
}

/// *Approved for every run* counts the project's rules on Runs and opens their page, where Revoke ends one and the
/// crumb leads back to Runs.
#[test]
fn project_settings_approved_for_every_run_opens_its_rules_and_revokes_one() {
  use orchester_core::{
    run::{ApprovalSubject, ProjectScope, ToolReach},
    time::Timestamp,
  };
  let mut page = Settings::new(false);
  assert!(page.text("settings-auto-rules-row").contains("No rules"));
  let rule = {
    let mut db = page.db();
    let run = super::run_fixture::started_run(&mut db);
    let scope = ProjectScope::of(&ApprovalSubject::McpTool {
      server: "Orchester".to_owned(),
      tool: "start_qa_app".to_owned(),
      reach: ToolReach::Script {
        blob: "3f2a1b9c0d4e5f60718293a4b5c6d7e8f9012345".to_owned(),
      },
    })
    .expect("a project rule");
    db.add_project_approval_rule(&scope, (run, "invocation:qa"), Timestamp::now())
      .expect("rule")
  };
  // Runs reads the settings again when it opens.
  page.tab("general", "settings-project-name");
  page.tab("runs", "settings-auto-rules-row");
  page.until("the rule counted", |p| {
    p.text("settings-auto-rules-row").contains("1 rule")
  });
  page.click("settings-auto-rules-row");
  let id = format!("settings-auto-rule-{}", rule.id.get());
  page.until("the rule listed", |p| p.exists(&format!("{id}-row")));
  assert_eq!(page.text("settings-rules-crumb-back"), "Runs");
  let words = page.text(&format!("{id}-row"));
  assert!(words.contains("Orchester start_qa_app"), "{words}");
  assert!(
    words.contains("while tools/qa-app.ps1 is 3f2a1b9 · answered 0 times"),
    "{words}"
  );
  assert!(
    page.exists("settings-rules-add"),
    "Add rule stays, unavailable with why"
  );
  assert!(page.text("settings-rules-how-reason").contains("Always allow"));
  page.click(&format!("{id}-revoke"));
  page.until("the rule revoked", |p| {
    p.db()
      .project_approval_rules()
      .expect("rules")
      .iter()
      .all(|rule| !rule.is_active())
  });
  page.until("the list read again", |p| !p.exists(&format!("{id}-row")));
  assert!(page.exists("settings-rules-none-row"));
  page.click("settings-rules-crumb-back");
  page.until("Runs", |p| p.exists("settings-auto-rules-row"));
}
