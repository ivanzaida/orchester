//! Tabs that live in a screen's route, each a place of its own and so an entry of the tab's history: they open by a
//! browser's rules too. One of each kind of screen on the QA seed's project: All projects | Usage, Project settings'
//! own tabs and its Secrets tab, and Instructions' scope tabs.

use orchester_app::routes::{InstructionsTab, InstructionsView, OverviewTab, ProjectPage, ProjectSettingsTab, Route};

use crate::{harness::Seeded, support::link_press::control_opens_by_browser_rules};

#[test]
fn all_projects_usage_tab_opens_by_browser_rules() {
  let mut seeded = Seeded::overview("tasks", OverviewTab::Running);
  seeded.window.navigation().go(Route::AllProjects);
  let place = Route::Usage(Box::default());
  control_opens_by_browser_rules(&mut seeded.window, "all-projects-tab-usage", &place);
}

#[test]
fn a_project_settings_tab_opens_by_browser_rules() {
  let mut seeded = Seeded::overview("tasks", OverviewTab::Running);
  let page = |tab| Route::Project(seeded.project, ProjectPage::Settings(tab));
  let (general, runs) = (page(ProjectSettingsTab::General), page(ProjectSettingsTab::Runs));
  seeded.window.navigation().go(general);
  control_opens_by_browser_rules(&mut seeded.window, "settings-tab-runs", &runs);
}

#[test]
fn the_project_settings_secrets_tab_opens_by_browser_rules() {
  let mut seeded = Seeded::overview("tasks", OverviewTab::Running);
  let general = Route::Project(seeded.project, ProjectPage::Settings(ProjectSettingsTab::General));
  let secrets = Route::Project(seeded.project, ProjectPage::Secrets(Default::default()));
  seeded.window.navigation().go(general);
  control_opens_by_browser_rules(&mut seeded.window, "settings-tab-secrets", &secrets);
}

#[test]
fn an_instructions_scope_tab_opens_by_browser_rules() {
  let mut seeded = Seeded::overview("tasks", OverviewTab::Running);
  let page = |tab| {
    Route::Project(
      seeded.project,
      ProjectPage::Instructions(InstructionsView::rule(tab, None)),
    )
  };
  let (project, roles) = (page(InstructionsTab::Project), page(InstructionsTab::Roles));
  seeded.window.navigation().go(project);
  control_opens_by_browser_rules(&mut seeded.window, "instruction-tab-1", &roles);
}
