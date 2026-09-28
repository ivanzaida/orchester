//! The tab strip over many tabs and across sessions (design doc, "Tabs"): tabs shrink, then the
//! strip scrolls and the tab list names every tab; a tab dragged along the strip lands where it
//! is dropped; the tooltip names the full location; and the tabs are saved and restored on the
//! next start, without their history.

use std::{fs, path::Path};

use lurq::app::events::MouseButton;
use orchester_store::AppDb;

use crate::{
  github::shows,
  harness::Desktop,
  tabs::{BACK, active_tab, click_with, command, has, side_button, tab_ids},
};

/// The saved tabs in the application database of `desktop`, as stored.
fn saved(repo: &Path) -> Option<String> {
  let database = repo
    .parent()
    .expect("the repository is in the test folder")
    .join("orchester.db");
  AppDb::open(&database).ok()?.ui_state("tabs").ok()?
}

/// Kontur on its task list in tab 1, Personal website opened in a background tab 2, and tab 2
/// active.
fn two_tabs() -> Desktop {
  let mut desktop = Desktop::with_other_project(|_| {}, |_| {});
  desktop.wait_for("nav-tasks");
  desktop.press("nav-tasks");
  desktop.wait_for("tasks-new-task");
  desktop.press("project-selector");
  desktop.wait_for("switcher-project-1");
  click_with(&mut desktop, "switcher-project-1", MouseButton::Left, (true, false));
  desktop.settle("tab 2", |tree| tree.get_element_by_id("window-tab-2").is_some());
  command(&mut desktop, "2", "Digit2", false);
  desktop.wait_for("project-selector");
  desktop
}

/// Waits until the database holds the tabs as the window shows them.
fn wait_saved(desktop: &mut Desktop, active: usize, paths: usize) {
  let repo = desktop.repo().to_path_buf();
  desktop.wait_until("the tabs saved", move |_| {
    saved(&repo).is_some_and(|text| {
      text.contains(&format!("active {active}\n")) && text.lines().filter(|line| line.starts_with('/')).count() == paths
    })
  });
}

#[test]
fn tabs_are_restored_on_the_next_start_without_their_history() {
  let mut desktop = two_tabs();
  wait_saved(&mut desktop, 1, 2);
  let mut desktop = desktop.crash_and_restart(|| {});
  desktop.settle("the restored tabs", |tree| {
    tree.get_element_by_id("window-tab-2").is_some()
  });
  assert_eq!(tab_ids(&mut desktop), [1, 2]);
  assert_eq!(active_tab(&mut desktop), Some(2), "the active tab is restored");
  desktop.wait_for("project-selector");
  assert!(shows(&mut desktop, "project-selector", "Personal website"));
  assert!(shows(&mut desktop, "window-tab-1", "Kontur · Tasks"));
  command(&mut desktop, "1", "Digit1", false);
  desktop.wait_for("tasks-new-task");
  side_button(&mut desktop, BACK);
  assert!(has(&mut desktop, "tasks-new-task"), "the history is not restored");
}

#[test]
fn tabs_a_restored_tab_whose_project_cannot_open_shows_all_projects() {
  let mut desktop = two_tabs();
  wait_saved(&mut desktop, 1, 2);
  let other = desktop.other_repo();
  let mut desktop = desktop.crash_and_restart(|| {
    fs::remove_dir_all(&other).expect("remove Personal website's folder");
  });
  desktop.settle("the restored tabs", |tree| {
    tree.get_element_by_id("window-tab-2").is_some()
  });
  assert_eq!(active_tab(&mut desktop), Some(2));
  desktop.wait_for("all-projects");
  assert!(shows(&mut desktop, "window-tab-2", "All projects"));
  command(&mut desktop, "1", "Digit1", false);
  desktop.wait_for("tasks-new-task");
}

/// Kontur and six more tabs on All projects in a window too narrow for them at 120px.
fn overflowing() -> Desktop {
  let mut desktop = Desktop::with_project();
  desktop.wait_for("nav-tasks");
  desktop.tree.resize(1100, 960);
  desktop.pass();
  for _ in 0..6 {
    command(&mut desktop, "t", "KeyT", false);
  }
  desktop.wait_for("all-projects");
  desktop
}

#[test]
fn tabs_overflow_scrolls_the_strip_and_lists_every_tab() {
  let mut desktop = overflowing();
  desktop.wait_for("window-tab-list");
  assert!(shows(&mut desktop, "window-tab-list", "7"));
  assert!(
    has(&mut desktop, "window-tab-fade-left"),
    "tabs are clipped on the left"
  );
  assert!(
    !has(&mut desktop, "window-tab-close-2"),
    "narrow inactive tabs drop their close button"
  );
  let active = active_tab(&mut desktop).expect("an active tab");
  assert!(has(&mut desktop, &format!("window-tab-close-{active}")));
  let tab = desktop.bounds(&format!("window-tab-{active}"));
  let strip = desktop.bounds("window-tab-viewport");
  assert!(
    tab.x + tab.width <= strip.x + strip.width + 0.5,
    "the active tab is in view"
  );
  let new_tab = desktop.bounds("window-new-tab");
  assert!(
    new_tab.x + new_tab.width + 48.0 <= 1100.0 - 138.0 + 0.5,
    "48px of drag area stay"
  );

  desktop.press("window-tab-list");
  desktop.wait_for("window-tab-list-menu");
  assert!(shows(&mut desktop, "window-tab-list-1", "Kontur · Overview"));
  desktop.press("window-tab-list-1");
  assert!(
    !has(&mut desktop, "window-tab-list-menu"),
    "choosing a tab closes the list"
  );
  assert_eq!(active_tab(&mut desktop), Some(1));
  desktop.wait_for("nav-tasks");
  assert!(
    has(&mut desktop, "window-tab-fade-right"),
    "switching scrolled the first tab into view"
  );

  desktop.press("window-tab-list");
  desktop.wait_for("window-tab-list-2");
  click_with(&mut desktop, "window-tab-list-2", MouseButton::Middle, (false, false));
  assert!(
    !tab_ids(&mut desktop).contains(&2),
    "a middle click in the list closes the tab"
  );
  desktop.key("Escape", "Escape");
  assert!(!has(&mut desktop, "window-tab-list-menu"), "Escape closes the list");
}

/// Drags `id` by `dx` along the strip, releasing at `release_y` from the window's top.
fn drag(desktop: &mut Desktop, id: &str, dx: f32, release_y: f32) {
  let bounds = desktop.bounds(id);
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  desktop.tree.mouse_down(x, y, MouseButton::Left);
  desktop.pass();
  for step in 1..=10 {
    desktop.tree.mouse_move(x + dx * step as f32 / 10.0, y);
    desktop.pass();
  }
  desktop.tree.mouse_up(x + dx, release_y, MouseButton::Left);
  desktop.pass();
}

/// Kontur and two tabs on All projects, the last one active.
fn three_tabs() -> Desktop {
  let mut desktop = Desktop::with_project();
  desktop.wait_for("nav-tasks");
  command(&mut desktop, "t", "KeyT", false);
  command(&mut desktop, "t", "KeyT", false);
  desktop.wait_for("all-projects");
  desktop
}

#[test]
fn tabs_drag_along_the_strip_to_reorder() {
  let mut desktop = three_tabs();
  assert_eq!(tab_ids(&mut desktop), [1, 2, 3]);
  drag(&mut desktop, "window-tab-1", 300.0, 16.0);
  assert_eq!(tab_ids(&mut desktop), [2, 1, 3], "the tab lands where it was dropped");
  // The dropped tab does not keep the pointer's hover: no tooltip comes after a rest.
  std::thread::sleep(std::time::Duration::from_millis(700));
  desktop.tree.tick_timers();
  desktop.pass();
  assert!(
    !has(&mut desktop, "window-tab-tooltip"),
    "no tooltip stays after the drop"
  );
  assert_eq!(active_tab(&mut desktop), Some(3), "dragging does not activate the tab");

  drag(&mut desktop, "window-tab-3", -500.0, 300.0);
  assert_eq!(
    tab_ids(&mut desktop),
    [2, 1, 3],
    "released away from the title bar, it goes back"
  );

  drag(&mut desktop, "window-tab-2", 1000.0, 16.0);
  assert_eq!(tab_ids(&mut desktop), [1, 3, 2], "dragged past the end, it lands last");
  drag(&mut desktop, "window-tab-2", -1000.0, 16.0);
  assert_eq!(tab_ids(&mut desktop), [2, 1, 3], "and past the start, first");

  let bounds = desktop.bounds("window-tab-2");
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  desktop.tree.mouse_down(x, y, MouseButton::Left);
  desktop.pass();
  desktop.tree.mouse_move(x + 400.0, y);
  desktop.pass();
  desktop.key("Escape", "Escape");
  desktop.tree.mouse_up(x + 400.0, y, MouseButton::Left);
  desktop.pass();
  assert_eq!(tab_ids(&mut desktop), [2, 1, 3], "Escape puts it back");
}

#[test]
fn tabs_tooltip_names_the_full_location() {
  let mut desktop = three_tabs();
  desktop.hover("window-tab-1");
  assert!(
    !has(&mut desktop, "window-tab-tooltip"),
    "the tooltip waits for the pointer to rest"
  );
  // The tooltip shows once the pointer rested on the tab; timers run with the window's loop.
  desktop.wait_until("the tooltip", |desktop| {
    desktop.tree.tick_timers();
    desktop.pass();
    has(desktop, "window-tab-tooltip")
  });
  assert!(shows(&mut desktop, "window-tab-tooltip", "Kontur · Overview"));
  desktop.hover("window-tab-2");
  assert!(
    !has(&mut desktop, "window-tab-tooltip"),
    "moving to another tab starts over"
  );
}
