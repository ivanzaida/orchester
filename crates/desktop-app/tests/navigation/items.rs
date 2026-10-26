//! An item of a project a person opens keeps each project in its own tab (`TabSet::open_item`): the current tab's
//! project's in the current tab, another project's in a tab of that project or in a new one right of the current
//! tab, never in place of the current tab; Back on the entry it arrived at returns to the tab it came from, once.

use orchester_app::{
  navigation::tabs::TabSet,
  routes::{OverviewTab, ProjectPage, Route, TaskTab, TaskView},
};

use super::project;

const ATLAS: &str = "0f1e2d3c-4b5a-4968-8776-655443322110";

fn atlas(page: ProjectPage) -> Route {
  Route::Project(ATLAS.parse().expect("a project id"), page)
}

fn overview() -> ProjectPage {
  ProjectPage::Overview(OverviewTab::default())
}

fn task(key: &str) -> ProjectPage {
  ProjectPage::Task(key.parse().expect("a task key"), TaskView::of(TaskTab::Summary))
}

#[test]
fn an_item_of_the_current_tabs_project_opens_in_the_current_tab() {
  let mut tabs = TabSet::at(project(overview()));
  let only = tabs.active().id();
  assert!(tabs.open_item(project(task("KON-12"))));
  assert_eq!((tabs.tabs().len(), tabs.active().id()), (1, only));
  assert_eq!(*tabs.active().route(), project(task("KON-12")));
  assert!(tabs.back());
  assert_eq!(
    *tabs.active().route(),
    project(overview()),
    "Back walks the tab's history"
  );
}

#[test]
fn another_projects_item_opens_in_a_new_tab_right_of_the_current_one_and_back_returns() {
  let mut tabs = TabSet::at(project(overview()));
  let kontur = tabs.active().id();
  let right = tabs.open_at(project(ProjectPage::Pipelines), false);
  assert!(tabs.open_item(atlas(task("ATL-3"))));
  let order: Vec<_> = tabs.tabs().iter().map(|tab| tab.id()).collect();
  assert_eq!(order.len(), 3);
  assert_eq!(order[0], kontur);
  assert_eq!(order[2], right, "the new tab is right of the current one");
  assert_eq!(tabs.active().id(), order[1]);
  assert_eq!(*tabs.active().route(), atlas(task("ATL-3")));
  assert_eq!(
    *tabs.tabs()[0].route(),
    project(overview()),
    "the current tab stays where it was"
  );
  assert!(tabs.can_back());
  assert!(tabs.back());
  assert_eq!(tabs.active().id(), kontur, "Back returns to the tab it came from");
  assert_eq!(*tabs.active().route(), project(overview()));
  assert!(tabs.select(order[1]));
  assert!(!tabs.can_back(), "the way back is used once");
  assert!(!tabs.back());
}

#[test]
fn another_projects_item_goes_to_a_tab_of_that_project() {
  let mut tabs = TabSet::at(project(overview()));
  let kontur = tabs.active().id();
  let atlas_tab = tabs.open_at(atlas(overview()), false);
  assert!(tabs.open_item(atlas(task("ATL-3"))));
  assert_eq!(tabs.tabs().len(), 2, "no new tab");
  assert_eq!(tabs.active().id(), atlas_tab);
  assert_eq!(*tabs.active().route(), atlas(task("ATL-3")));
  assert_eq!(*tabs.tabs()[0].route(), project(overview()));
  assert!(tabs.back());
  assert_eq!(tabs.active().id(), kontur, "Back returns to the tab it came from first");
  assert!(tabs.select(atlas_tab));
  assert!(tabs.back());
  assert_eq!(
    (tabs.active().id(), tabs.active().route()),
    (atlas_tab, &atlas(overview())),
    "then the tab's own history"
  );
}

#[test]
fn a_tab_that_shows_no_project_goes_to_the_item_itself_unless_its_project_has_a_tab() {
  let mut tabs = TabSet::default();
  assert!(tabs.open_item(atlas(task("ATL-3"))));
  assert_eq!(tabs.tabs().len(), 1, "All projects becomes the project's tab");
  let mut tabs = TabSet::default();
  let home = tabs.active().id();
  let atlas_tab = tabs.open_at(atlas(overview()), false);
  assert!(tabs.open_item(atlas(task("ATL-3"))));
  assert_eq!(tabs.active().id(), atlas_tab);
  assert!(tabs.back());
  assert_eq!(tabs.active().id(), home);
}

#[test]
fn the_way_back_follows_the_entry_and_goes_with_the_tab_it_came_from() {
  let mut tabs = TabSet::at(project(overview()));
  let kontur = tabs.active().id();
  tabs.open_item(atlas(task("ATL-3")));
  assert!(tabs.go(atlas(task("ATL-4"))));
  assert!(tabs.back());
  assert_eq!(*tabs.active().route(), atlas(task("ATL-3")), "Back walks the tab first");
  assert!(tabs.back());
  assert_eq!(
    tabs.active().id(),
    kontur,
    "then from the entry it arrived at to the tab it came from"
  );
  let mut tabs = TabSet::at(project(overview()));
  let kontur = tabs.active().id();
  tabs.open_item(atlas(task("ATL-3")));
  assert!(tabs.close(kontur));
  assert!(!tabs.can_back(), "the tab it came from is closed");
  assert!(!tabs.back());
}
