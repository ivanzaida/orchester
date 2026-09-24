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
  // The selected Overview and the screens that do not exist yet are skipped. The status bar
  // items that open something are stops: the runner (whose failure actions appear once the
  // missing runner gives up), then the agents and the queue, the last stop before Tab wraps.
  assert_eq!(
    desktop.stops(6, false),
    expected(&[
      Some("nav-tasks"),
      Some("nav-pipelines"),
      Some("nav-agents"),
      Some("nav-connections"),
      Some("new-task"),
      Some("status-runner"),
    ])
  );
  desktop.tab_to("status-queued", MAX_STOPS);
  assert_eq!(desktop.stops(1, false), expected(&[Some("nav-tasks")]));
  assert_eq!(desktop.stops(1, true), expected(&[Some("status-queued")]));
}

#[test]
fn sidebar_enter_opens_the_focused_screen() {
  let mut desktop = overview();
  desktop.tab_to("nav-pipelines", MAX_STOPS);
  desktop.enter();
  desktop.wait_for("pipelines-new");
  desktop.tab_to("nav-overview", MAX_STOPS);
  desktop.key(" ", "Space");
  desktop.wait_for("new-task");
}

#[test]
fn tasks_tab_row_selects_from_the_keyboard() {
  let mut desktop = overview();
  desktop.click("nav-tasks");
  desktop.wait_for("tasks-new-task");
  // All tasks is selected, so only Drafts is a stop in the tab row.
  desktop.tab_to("tasks-tab-drafts", MAX_STOPS);
  desktop.enter();
  // Drafts is selected now and All tasks can be chosen instead; the empty Drafts list offers
  // New task next.
  desktop.tab_to("tasks-tab-all", MAX_STOPS);
  assert_eq!(desktop.stops(1, false), expected(&[Some("empty-new-task")]));
}
