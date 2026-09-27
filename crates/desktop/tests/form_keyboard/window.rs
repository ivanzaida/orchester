use orchester_desktop::{activity::OperationSpec, messages::Message};

use crate::{github::shows, harness::Desktop};

/// More than the window has stops on these screens.
const MAX_STOPS: usize = 30;
const TITLE: &str = "task-title";

fn overview() -> Desktop {
  let mut desktop = Desktop::with_project();
  desktop.wait_for("nav-tasks");
  desktop
}

#[test]
fn overview_without_a_form_has_no_tab_stops() {
  let mut desktop = overview();
  // The sidebar, the tabs, New task, View all tasks and the status bar are pointer-only.
  assert!(desktop.stops(MAX_STOPS, false).iter().all(Option::is_none));
  assert!(desktop.stops(MAX_STOPS, true).iter().all(Option::is_none));
}

/// Lucide's `circle-check` (a check inside a circle, `sQnmq` in `KYnRz`) is `check-circle-2` in
/// the bundled font; the font's `check-circle` (U+E07C) is today's `circle-check-big`.
#[test]
fn tasks_item_draws_a_check_inside_a_circle() {
  let mut desktop = overview();
  assert!(shows(&mut desktop, "nav-tasks", "\u{e226}"));
  assert!(!shows(&mut desktop, "nav-tasks", "\u{e07c}"));
}

#[test]
fn clicking_a_nav_item_opens_its_screen_without_focusing_it() {
  let mut desktop = overview();
  desktop.press("nav-pipelines");
  desktop.wait_for("pipelines-new");
  assert_eq!(desktop.focused(), None);
}

#[test]
fn clicking_a_tab_selects_it_without_focusing_it() {
  let mut desktop = overview();
  desktop.click("nav-tasks");
  desktop.wait_for("tasks-new-task");
  desktop.press("tasks-tab-drafts");
  desktop.wait_for("empty-new-task");
  assert_eq!(desktop.focused(), None);
}

#[test]
fn pressing_an_unavailable_item_clears_the_focus() {
  let mut desktop = overview();
  desktop.click("nav-tasks");
  desktop.wait_for("tasks-new-task");
  desktop.click("tasks-new-task");
  desktop.wait_for(TITLE);
  desktop.tab_to(TITLE, MAX_STOPS);
  // Tools does not exist yet: a press on it is a press on empty space, which clears the focus.
  desktop.press("nav-tools");
  assert_eq!(desktop.focused(), None);
}

#[test]
fn escape_closes_the_operations_popover_without_focusing_the_activity_item() {
  // Without a project nothing opens a failure, so the activity item opens the popover, whose
  // row for the failure has a dismiss button.
  let mut desktop = Desktop::first_launch();
  desktop.wait_for("create-first-project");
  let failed = Message::new("statusBar.operations.title");
  desktop
    .activity
    .start(OperationSpec::new(failed.clone(), failed.clone(), failed.clone()))
    .fail(failed);
  desktop.wait_for("status-activity");
  desktop.press("status-activity");
  desktop.wait_for("status-operations");
  assert_eq!(desktop.focused(), None);
  desktop.key("Escape", "Escape");
  assert!(desktop.tree.get_element_by_id("status-operations").is_none());
  assert_eq!(desktop.focused(), None);
}
