//! The Agents hub of a local project: Instructions, Skills, Tools and Memory are tabs of their own pages, each with its
//! own route; Profiles is the master-detail Agents page (`local_hub`).
use orchester_app::routes::{InstructionsView, MemoryView, ProjectPage, Route, SkillsFilter};

use super::{harness::Agents, support::project_id};

#[test]
fn hub_tabs_open_each_tab_on_its_own_route() {
  let mut screen = Agents::new(true);
  screen
    .window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Tools));
  screen.until("tools", |s| s.exists("agents-hub-tabs") && s.exists("tools-add"));
  // The hub's tabs are 32 high (`Cc3JP`), not the 34 of a page's tabs.
  let tab = screen
    .window
    .tree
    .get_element_by_id_mut("agents-hub-tab-tools")
    .and_then(|tab| tab.bounds())
    .expect("the Tools tab is laid out");
  assert!((tab.height - 32.0).abs() < 0.5, "{tab:?}");
  let tabs = [
    ("instructions", ProjectPage::Instructions(InstructionsView::default())),
    ("skills", ProjectPage::Skills(SkillsFilter::default())),
    ("tools", ProjectPage::Tools),
  ];
  for (tab, page) in tabs {
    screen.click(&format!("agents-hub-tab-{tab}"));
    let route = Route::Project(project_id(), page);
    screen.until(tab, |s| {
      s.window.navigation().route_now() == route && s.exists("agents-hub-title")
    });
    assert_eq!(screen.text("agents-hub-title"), "Agents");
  }
  // Memory is the master-detail hub's filter now: the old tabs' Memory lands on it.
  screen.click("agents-hub-tab-memory");
  let memory = Route::Project(project_id(), ProjectPage::Memory(MemoryView::default()));
  screen.until("memory", |s| {
    s.window.navigation().route_now() == memory && s.exists("local-hub-filter-memory")
  });
  screen.click("agents-hub-tab-tools");
  screen.until("tools", |s| s.exists("agents-hub-title"));
  // Profiles is the local project's master-detail page, which has its own head.
  screen.click("agents-hub-tab-profiles");
  screen.until("profiles", |s| s.exists("screen-local-hub"));
  assert_eq!(screen.text("local-hub-title"), "Agents");
}

#[test]
fn hub_head_holds_the_open_tabs_actions() {
  let mut screen = Agents::new(true);
  screen.window.navigation().go(Route::Project(
    project_id(),
    ProjectPage::Skills(SkillsFilter::default()),
  ));
  screen.until("skills", |s| s.exists("skills-new"));
  screen.click("agents-hub-tab-tools");
  screen.until("tools", |s| s.exists("tools-add"));
  assert!(!screen.exists("skills-new"));
  screen.click("agents-hub-tab-memory");
  screen.until("memory", |s| s.exists("memory-new"));
}
