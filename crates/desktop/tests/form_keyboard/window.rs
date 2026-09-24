use orchester_desktop::{activity::OperationSpec, messages::Message};

use crate::harness::{Desktop, expected};

/// More than the window has stops on these screens.
const MAX_STOPS: usize = 30;

fn overview() -> Desktop {
  let mut desktop = Desktop::with_project();
  desktop.wait_for("nav-tasks");
  desktop
}

#[test]
fn overview_tab_reaches_sidebar_page_and_status_bar() {
  let mut desktop = overview();
  // The selected Overview stays a stop; the screens that do not exist yet are skipped. The
  // status bar items that open something are stops: the runner (whose failure actions appear
  // once the missing runner gives up), then the agents and the queue, the last stop before Tab
  // wraps.
  assert_eq!(
    desktop.stops(7, false),
    expected(&[
      Some("nav-overview"),
      Some("nav-tasks"),
      Some("nav-pipelines"),
      Some("nav-agents"),
      Some("nav-connections"),
      Some("new-task"),
      Some("status-runner"),
    ])
  );
  desktop.tab_to("status-queued", MAX_STOPS);
  assert_eq!(desktop.stops(1, false), expected(&[Some("nav-overview")]));
  assert_eq!(desktop.stops(1, true), expected(&[Some("status-queued")]));
}

#[test]
fn sidebar_enter_opens_the_focused_screen() {
  let mut desktop = overview();
  desktop.tab_to("nav-pipelines", MAX_STOPS);
  desktop.enter();
  desktop.wait_for("pipelines-new");
  // The opened screen's item is selected now and keeps the focus.
  assert_eq!(desktop.focused().as_deref(), Some("nav-pipelines"));
  desktop.tab_to("nav-overview", MAX_STOPS);
  desktop.key(" ", "Space");
  desktop.wait_for("new-task");
}

#[test]
fn tasks_tab_row_selects_from_the_keyboard() {
  let mut desktop = overview();
  desktop.click("nav-tasks");
  desktop.wait_for("tasks-new-task");
  // Both tabs are stops, the selected All tasks too.
  desktop.tab_to("tasks-tab-all", MAX_STOPS);
  assert_eq!(desktop.stops(1, false), expected(&[Some("tasks-tab-drafts")]));
  desktop.enter();
  // Drafts is selected now and keeps the focus; the empty Drafts list offers New task next.
  assert_eq!(desktop.focused().as_deref(), Some("tasks-tab-drafts"));
  assert_eq!(desktop.stops(1, false), expected(&[Some("empty-new-task")]));
}

#[test]
fn pressing_an_unavailable_item_clears_the_focus() {
  let mut desktop = overview();
  desktop.tab_to("nav-tasks", MAX_STOPS);
  // Tools does not exist yet: it cannot take the focus, so the press clears it.
  desktop.press("nav-tools");
  assert_eq!(desktop.focused(), None);
}

#[test]
fn escape_in_the_operations_popover_returns_the_focus_to_the_activity_item() {
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
  desktop.tab_to("status-activity", MAX_STOPS);
  desktop.enter();
  desktop.wait_for("status-operations");
  desktop.tab_to("status-operation-dismiss", MAX_STOPS);
  desktop.key("Escape", "Escape");
  assert!(desktop.tree.get_element_by_id("status-operations").is_none());
  assert_eq!(desktop.focused().as_deref(), Some("status-activity"));
}
