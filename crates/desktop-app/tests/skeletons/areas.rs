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
fn skeletons_review_shows_the_merge_queues_and_the_pull_requests_bones_after_a_moment() {
  let mut held = Held::new();
  held.page(ProjectPage::MergeQueue);
  held.skeleton("merge-queue-loading");
  assert!(held.exists("review-hub-title"), "the hub's title stays real");
  held.page(ProjectPage::PullRequests(Default::default()));
  held.skeleton("pull-requests-loading");
  assert!(held.exists("pull-requests-refresh"), "Refresh stays real");
  assert!(
    held.exists("pull-requests-repository-loading-bones") && held.exists("pull-requests-sync-status-loading-bones"),
    "the repository and the sync line are bones in their own lines"
  );
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
fn skeletons_project_settings_shows_the_tabs_bones_after_a_moment() {
  let mut held = Held::new();
  held.page(ProjectPage::Settings(ProjectSettingsTab::General));
  held.skeleton("settings-loading");
}

#[test]
fn skeletons_app_settings_shows_the_accounts_bones_after_a_moment() {
  let mut held = Held::new();
  held.go(Route::Accounts(AccountsView::default()));
  held.skeleton("settings-accounts-loading");
}
