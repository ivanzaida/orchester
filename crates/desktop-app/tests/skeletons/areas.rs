//! Each area's skeleton while its reads are held: none of its bones before the delay, its bones after it.

use orchester_app::routes::{
  AccountsView, FilesView, InstructionsView, ModuleListTab, OrchestratorChat, OverviewTab, ProjectPage,
  ProjectSettingsTab, Route, TaskView,
};

use crate::harness::Held;

#[test]
fn skeletons_overview_shows_its_sections_bones_after_a_moment() {
  let mut held = Held::new();
  held.page(ProjectPage::Overview(OverviewTab::default()));
  held.skeleton("overview-waits-bones");
  assert!(held.exists("overview-header"), "the header stands over the bones");
}

#[test]
fn skeletons_task_page_shows_its_header_and_summary_bones_after_a_moment() {
  let mut held = Held::new();
  let task = held.task.clone();
  held.page(ProjectPage::Task(task, TaskView::Summary));
  held.skeleton("task-loading");
  assert!(held.exists("task-tab-summary"), "the tabs stay real");
}

#[test]
fn skeletons_orchestrator_shows_the_conversations_bones_after_a_moment() {
  let mut held = Held::new();
  held.page(ProjectPage::Orchestrator(OrchestratorChat::Latest));
  held.skeleton("orchestrator-loading");
}

#[test]
fn skeletons_files_shows_the_trees_bones_after_a_moment() {
  let mut held = Held::new();
  held.page(ProjectPage::Files(FilesView::default()));
  held.skeleton("files-tree-loading");
}

#[test]
fn skeletons_modules_shows_the_lists_bones_after_a_moment() {
  let mut held = Held::new();
  held.page(ProjectPage::Modules(ModuleListTab::Open));
  held.skeleton("modules-loading");
  assert!(held.exists("modules-search"), "the search stands where it will");
}

#[test]
fn skeletons_usage_shows_the_charts_bones_after_a_moment() {
  let mut held = Held::new();
  held.page(ProjectPage::Usage(Box::default()));
  held.skeleton("usage-tokens-reading");
  assert!(held.exists("usage-limits-reading"), "the plan limits read on their own");
}

#[test]
fn skeletons_all_projects_shows_the_rows_bones_after_a_moment() {
  let mut held = Held::new();
  held.go(Route::AllProjects);
  held.skeleton("all-projects-loading");
  assert!(held.exists("all-projects-title"), "the title stays real");
}

#[test]
fn skeletons_pipelines_shows_the_librarys_bones_after_a_moment() {
  let mut held = Held::new();
  held.page(ProjectPage::Pipelines);
  held.skeleton("pipelines-loading");
}

#[test]
fn skeletons_agents_shows_the_profiles_bones_after_a_moment() {
  let mut held = Held::new();
  held.page(ProjectPage::Agents);
  held.skeleton("agents-loading");
  assert!(held.exists("agents-hub-tab-profiles"), "the hub's tabs stay real");
}

#[test]
fn skeletons_agents_profile_keeps_the_heads_buttons() {
  let mut held = Held::new();
  held.page(ProjectPage::Agents);
  held.skeleton("agents-loading");
  held.click("agents-new");
  held.until("agents-reading-page");
  assert!(
    held.exists("agents-import") && held.exists("agents-new"),
    "Import agents and New agent stay real over a profile's bones"
  );
}

#[test]
fn skeletons_instruction_import_shows_the_reviews_bones_after_a_moment() {
  let mut held = Held::new();
  held.hold.set(false);
  held.page(ProjectPage::Instructions(InstructionsView::default()));
  held.until("instructions-new");
  held.window.until("the instructions are read", |window| {
    window.tree.get_element_by_id("instructions-loading").is_none()
  });
  held.hold.set(true);
  held.page(ProjectPage::Instructions(InstructionsView::Import));
  held.skeleton("instruction-import-loading");
  assert!(held.exists("instruction-import-title"), "the page's title stays real");
  assert!(
    held.exists("instruction-import-notice"),
    "the notice's fixed words stay real"
  );
  held.release("instruction-import-create");
  assert!(
    !held.exists("instruction-import-loading"),
    "the review takes the bones' place"
  );
}
#[test]
fn skeletons_project_settings_shows_the_tabs_bones_from_the_first_frame() {
  let mut held = Held::new();
  held.page(ProjectPage::Settings(ProjectSettingsTab::General));
  held.skeleton_at_once("settings-loading");
}

#[test]
fn skeletons_app_settings_shows_the_accounts_bones_from_the_first_frame() {
  let mut held = Held::new();
  held.go(Route::Accounts(AccountsView::default()));
  held.skeleton_at_once("settings-accounts-loading");
}

#[test]
fn skeletons_app_settings_browser_shows_its_groups_bones_while_the_browser_is_looked_for() {
  let mut held = Held::new();
  held.go(Route::Browser);
  held.skeleton_at_once("settings-browser-reading");
  for real in [
    "settings-browser-program-reading-title",
    "settings-browser-found-reading-label",
    "settings-browser-found-reading-hint",
    "settings-browser-program-reading-hint",
    "settings-browser-profiles-reading-hint",
    "settings-browser-window-reading-label",
  ] {
    assert!(held.exists(real), "{real}: headings, labels and help lines stay real");
  }
  for later in [
    "settings-browser-choose",
    "settings-browser-window",
    "settings-browser-groups",
  ] {
    assert!(!held.exists(later), "{later} comes with what is read");
  }
  held.release("settings-browser-groups");
  assert!(
    !held.exists("settings-browser-reading"),
    "the groups take the bones' place"
  );
}

#[test]
fn skeletons_app_settings_daily_report_shows_no_switch_before_its_setting_is_read() {
  let mut held = Held::new();
  held.go(Route::Notifications);
  held.skeleton_at_once("settings-daily-report-reading");
  assert!(held.exists("settings-daily-report-row"), "the row's label stays real");
  assert!(
    held.exists("settings-daily-report-at-reading"),
    "the time is a bone too"
  );
  assert!(
    !held.exists("settings-daily-report"),
    "no switch before the setting is read"
  );
  held.release("settings-daily-report");
  assert!(!held.exists("settings-daily-report-reading"));
  assert!(
    !held.exists("settings-daily-report-at-row"),
    "off until turned on: no time once it is read"
  );
}
