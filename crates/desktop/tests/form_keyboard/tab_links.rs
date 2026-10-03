//! Links inside a page open where the press asks (design doc, "Tabs" › "Opening and closing"):
//! Ctrl/Cmd+click and a middle click open the place in a new tab behind the current one,
//! Ctrl/Cmd+Shift+click in a new tab that becomes active, a plain click in the current tab. Enter
//! on a focused link stays in the current tab whatever keys are held.

use std::{path::Path, time::Duration};

use lurq::app::events::MouseButton;
use orchester_core::{
  id::IdempotencyKey,
  store::{RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  tokens::TokenBudget,
};
use orchester_store::ProjectDb;

use crate::{
  agents::with_connection,
  github::shows,
  harness::Desktop,
  pull_request_detail::signed_in_with_detail,
  pull_requests::{linked, open_pull_requests, settle_on},
  tabs::{active_tab, click_with, command, has, tab_ids},
  unreadable_run::{at, delivery, name, started},
};

/// More than the new profile form has stops.
const MAX_STOPS: usize = 30;

/// KON-1 "Add repository switcher" with its cancelled run #1 of the published pipeline Delivery.
fn seed(repo: &Path) {
  let mut db = ProjectDb::open(repo).expect("open the project database");
  let snapshot = delivery(&mut db);
  let details = TaskDetails {
    title: name("Add repository switcher"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: Some(
      TaskLimits::new(
        TokenBudget::new(1_000_000).expect("above zero"),
        Duration::from_secs(1_800),
      )
      .expect("limits"),
    ),
  };
  let task = db
    .create_tasks(IdempotencyKey::generate(), vec![details], at(5))
    .expect("task")[0]
    .id();
  let mut run = started(&mut db, (task, &snapshot), 1, 10);
  let events = run.cancel(at(20)).expect("cancel");
  db.append_events(&run, &events).expect("append");
}

/// Kontur with KON-1, on its overview.
fn kontur() -> Desktop {
  let mut desktop = Desktop::with_seeded_project(seed);
  desktop.wait_for("nav-tasks");
  desktop
}

/// The page `reach` shows in the first tab holds `link`, whose place `arrived` marks and whose tab
/// is labelled `label`. Ctrl/Cmd+click and a middle click open it in background tabs, Ctrl/Cmd+Shift+
/// click in a new active tab, and a plain click in the first tab.
fn opens_each_way(desktop: &mut Desktop, reach: impl Fn(&mut Desktop), link: &str, (arrived, label): (&str, &str)) {
  reach(desktop);
  desktop.wait_for(link);

  click_with(desktop, link, MouseButton::Left, (true, false));
  desktop.settle("a background tab", |tree| {
    tree.get_element_by_id("window-tab-2").is_some()
  });
  assert_eq!(active_tab(desktop), Some(1), "Ctrl/Cmd+click does not switch");
  assert!(has(desktop, link), "the page stays where it was");
  assert!(shows(desktop, "window-tab-2", label));

  click_with(desktop, link, MouseButton::Middle, (false, false));
  desktop.settle("another background tab", |tree| {
    tree.get_element_by_id("window-tab-3").is_some()
  });
  assert_eq!(active_tab(desktop), Some(1), "a middle click does not switch");
  assert!(shows(desktop, "window-tab-3", label));

  click_with(desktop, link, MouseButton::Left, (true, true));
  desktop.settle("an active new tab", |tree| {
    tree.get_element_by_id("window-tab-4").is_some()
  });
  assert_eq!(active_tab(desktop), Some(4), "Ctrl/Cmd+Shift+click switches");
  desktop.wait_for(arrived);

  command(desktop, "1", "Digit1", false);
  desktop.wait_for(link);
  click_with(desktop, link, MouseButton::Left, (false, false));
  desktop.wait_for(arrived);
  assert_eq!(active_tab(desktop), Some(1), "a plain click opens it here");
  assert_eq!(tab_ids(desktop).len(), 4, "a plain click opens no tab");
}

#[test]
fn tab_links_task_row() {
  let mut desktop = kontur();
  let tasks = |desktop: &mut Desktop| desktop.press("nav-tasks");
  opens_each_way(&mut desktop, tasks, "task-row-KON-1", ("task-page", "Kontur · KON-1"));
}

#[test]
fn tab_links_pipeline_row() {
  let mut desktop = kontur();
  let pipelines = |desktop: &mut Desktop| desktop.press("nav-pipelines");
  opens_each_way(
    &mut desktop,
    pipelines,
    "pipeline-row",
    ("pipeline-breadcrumb-library", "Kontur · Delivery"),
  );
}

#[test]
fn tab_links_overview_activity_card() {
  let mut desktop = kontur();
  // Recent activity comes with Insights (ORC-39) while nothing needs the person.
  let overview = |desktop: &mut Desktop| {
    desktop.wait_for("overview-insights");
    desktop.click("overview-insights");
  };
  opens_each_way(
    &mut desktop,
    overview,
    "overview-event-KON-1",
    ("task-page", "Kontur · KON-1"),
  );
}

#[test]
fn tab_links_status_bar_item() {
  let mut desktop = kontur();
  let overview = |_: &mut Desktop| {};
  opens_each_way(
    &mut desktop,
    overview,
    "status-queued",
    ("tasks-new-task", "Kontur · Tasks"),
  );
}

#[test]
fn tab_links_breadcrumb() {
  let mut desktop = kontur();
  let task = |desktop: &mut Desktop| {
    desktop.press("nav-tasks");
    desktop.wait_for("task-row-KON-1");
    desktop.press("task-row-KON-1");
    desktop.wait_for("task-page");
  };
  opens_each_way(
    &mut desktop,
    task,
    "task-breadcrumb",
    ("tasks-new-task", "Kontur · Tasks"),
  );
}

#[test]
fn tab_links_pull_request_row() {
  let gh = signed_in_with_detail();
  let (mut desktop, _) = linked(&gh, "ivanzaida");
  let inbox = |desktop: &mut Desktop| {
    open_pull_requests(desktop);
    settle_on(desktop, "pull-requests-list", "Add GitHub authentication");
  };
  opens_each_way(
    &mut desktop,
    inbox,
    "pull-request-row",
    ("pull-request", "Kontur · #42"),
  );
}

/// Enter on a focused link that is a form control (*Add MCP server* on a new profile's Permissions
/// tab) opens its place in the current tab with Ctrl/Cmd held, also right after a Ctrl/Cmd+click
/// on a link.
#[test]
fn tab_links_enter_stays_in_the_current_tab() {
  let mut desktop = with_connection();
  desktop.wait_for("nav-agents");
  desktop.press("nav-agents");
  desktop.wait_for("empty-new-agent");
  desktop.press("empty-new-agent");
  desktop.wait_for("agent-tab-permissions");
  desktop.press("agent-tab-permissions");
  desktop.wait_for("agent-mcp-add");
  click_with(&mut desktop, "nav-tasks", MouseButton::Left, (true, false));
  desktop.settle("a background tab", |tree| {
    tree.get_element_by_id("window-tab-2").is_some()
  });
  desktop.tab_to("agent-mcp-add", MAX_STOPS);
  if cfg!(target_os = "macos") {
    desktop
      .tree
      .key_down_with_meta("Enter".into(), "Enter".into(), false, false, false, true);
  } else {
    desktop
      .tree
      .key_down("Enter".into(), "Enter".into(), false, true, false);
  }
  desktop.pass();
  desktop.wait_for("connections-tab-mcp");
  assert!(
    !has(&mut desktop, "agent-mcp-add"),
    "the connections replaced the profile"
  );
  assert_eq!(tab_ids(&mut desktop), [1, 2], "Enter opens no tab");
  assert_eq!(active_tab(&mut desktop), Some(1));
}

/// A held Ctrl/Cmd repeats its key press while the pointer is down; the click still opens a
/// background tab.
#[test]
fn tab_links_a_repeating_modifier_keeps_the_click_a_pointer_click() {
  let mut desktop = kontur();
  desktop.press("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  let bounds = desktop.bounds("task-row-KON-1");
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  let mac = cfg!(target_os = "macos");
  let (key, code) = if mac {
    ("Meta", "MetaLeft")
  } else {
    ("Control", "ControlLeft")
  };
  let held = |desktop: &mut Desktop| {
    desktop
      .tree
      .key_down_with_meta(key.into(), code.into(), false, !mac, false, mac);
  };
  held(&mut desktop);
  desktop
    .tree
    .mouse_down_with_modifiers(x, y, MouseButton::Left, false, !mac, false);
  held(&mut desktop);
  desktop
    .tree
    .mouse_up_with_modifiers(x, y, MouseButton::Left, false, !mac, false);
  desktop
    .tree
    .key_up_with_meta(key.into(), code.into(), false, false, false, false);
  desktop.pass();
  desktop.settle("a background tab", |tree| {
    tree.get_element_by_id("window-tab-2").is_some()
  });
  assert_eq!(active_tab(&mut desktop), Some(1));
  assert!(has(&mut desktop, "task-row-KON-1"));
}
