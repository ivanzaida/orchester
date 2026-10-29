//! Each area's skeleton while its reads are held: none of its bones before the delay, its bones after it.

use orchester_app::routes::{
  AccountsView, FilesView, ModuleListTab, OrchestratorChat, OverviewTab, ProjectPage, ProjectSettingsTab, Route,
  TaskView,
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
