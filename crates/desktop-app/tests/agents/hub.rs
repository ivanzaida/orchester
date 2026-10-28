//! The Agents hub: Profiles, Instructions, Skills, Tools and Memory are tabs of one page, each with its own route.
use orchester_app::routes::{InstructionsView, ProjectPage, Route, SkillsFilter};

use super::{harness::Agents, support::project_id};

#[test]
fn hub_tabs_open_each_tab_on_its_own_route() {
  let mut screen = Agents::new(true);
  screen.go();
  assert!(screen.exists("agents-hub-tabs"));
  // The hub's tabs are 32 high (`Cc3JP`), not the 34 of a page's tabs.
  let tab = screen
    .window
    .tree
    .get_element_by_id_mut("agents-hub-tab-profiles")
    .and_then(|tab| tab.bounds())
    .expect("the Profiles tab is laid out");
  assert!((tab.height - 32.0).abs() < 0.5, "{tab:?}");
  let tabs = [
    ("instructions", ProjectPage::Instructions(InstructionsView::default())),
    ("skills", ProjectPage::Skills(SkillsFilter::default())),
    ("tools", ProjectPage::Tools),
    ("memory", ProjectPage::Memory(None)),
    ("profiles", ProjectPage::Agents),
  ];
  for (tab, page) in tabs {
    screen.click(&format!("agents-hub-tab-{tab}"));
    let route = Route::Project(project_id(), page);
    screen.until(tab, |s| {
      s.window.navigation().route_now() == route && s.exists("agents-hub-title")
    });
    assert_eq!(screen.text("agents-hub-title"), "Agents");
  }
}

#[test]
fn hub_head_holds_the_open_tabs_actions() {
  let mut screen = Agents::new(true);
  screen.go();
  assert!(screen.exists("agents-import") && screen.exists("agents-new"));
  screen.click("agents-hub-tab-skills");
  screen.until("skills", |s| s.exists("skills-new"));
  assert!(screen.exists("skills-import-open") && !screen.exists("agents-new"));
  screen.click("agents-hub-tab-tools");
  screen.until("tools", |s| s.exists("tools-add"));
  assert!(screen.exists("tools-import"));
  screen.click("agents-hub-tab-memory");
  screen.until("memory", |s| s.exists("memory-new"));
}
