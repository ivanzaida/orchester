//! Tabs in the title bar (design doc, "Tabs"): + and Ctrl+T open All projects in a new tab, the ×
//! and a middle click close one, closing the last tab leaves All projects, the shortcuts switch
//! between tabs, Ctrl/Cmd+click and a middle click open projects in new tabs from the switcher and
//! All projects, each tab has its own Back/Forward, an inactive tab keeps its page as it was, a tab
//! with unsaved edits asks before it closes, and the tabs do not react under a dialog.

use lurq::app::events::{MouseButton, ScrollPhase};

use crate::{
  github::{contains_text, shows},
  harness::Desktop,
};

pub(crate) const BACK: MouseButton = MouseButton::Other(4);

pub(crate) fn has(desktop: &mut Desktop, id: &str) -> bool {
  desktop.tree.get_element_by_id(id).is_some()
}

/// The ids of the open tabs, in their order in the strip.
pub(crate) fn tab_ids(desktop: &mut Desktop) -> Vec<u64> {
  let mut tabs: Vec<(f32, u64)> = (1..64)
    .filter_map(|id| {
      let bounds = desktop
        .tree
        .get_element_by_id_mut(&format!("window-tab-{id}"))
        .and_then(|element| element.bounds())?;
      Some((bounds.x, id))
    })
    .collect();
  tabs.sort_by(|left, right| left.0.total_cmp(&right.0));
  tabs.into_iter().map(|(_, id)| id).collect()
}

/// The id of the active tab.
pub(crate) fn active_tab(desktop: &mut Desktop) -> Option<u64> {
  let active = desktop.tree.get_elements_by_class_name("window-tab-active");
  active
    .first()
    .and_then(|tab| tab.id())
    .and_then(|id| id.strip_prefix("window-tab-"))
    .and_then(|id| id.parse().ok())
}

/// Ctrl (Cmd on macOS) with `key`, and Shift when asked.
pub(crate) fn command(desktop: &mut Desktop, key: &str, code: &str, shift: bool) {
  if cfg!(target_os = "macos") {
    desktop
      .tree
      .key_down_with_meta(key.into(), code.into(), shift, false, false, true);
  } else {
    desktop.tree.key_down(key.into(), code.into(), shift, true, false);
  }
  desktop.pass();
}

/// A click in the middle of `id` with `button`, Ctrl/Cmd and Shift as asked.
pub(crate) fn click_with(desktop: &mut Desktop, id: &str, button: MouseButton, (command, shift): (bool, bool)) {
  let bounds = desktop.bounds(id);
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  if cfg!(target_os = "macos") && command {
    // Cmd reaches a click through the window's key events (lurq's mouse events carry no Cmd).
    desktop
      .tree
      .key_down_with_meta("Meta".into(), "MetaLeft".into(), false, false, false, true);
  }
  let ctrl = command && !cfg!(target_os = "macos");
  desktop.tree.mouse_down_with_modifiers(x, y, button, shift, ctrl, false);
  desktop.tree.mouse_up_with_modifiers(x, y, button, shift, ctrl, false);
  if cfg!(target_os = "macos") && command {
    desktop
      .tree
      .key_up_with_meta("Meta".into(), "MetaLeft".into(), false, false, false, false);
  }
  desktop.pass();
}

pub(crate) fn side_button(desktop: &mut Desktop, button: MouseButton) {
  desktop.tree.mouse_down(720.0, 480.0, button);
  desktop.pass();
  desktop.tree.mouse_up(720.0, 480.0, button);
  desktop.pass();
}

/// Kontur on its overview, in one tab.
fn kontur() -> Desktop {
  let mut desktop = Desktop::with_project();
  desktop.wait_for("nav-tasks");
  desktop
}

/// Kontur shown, Personal website opened before it.
fn two_projects() -> Desktop {
  let mut desktop = Desktop::with_other_project(|_| {}, |_| {});
  desktop.wait_for("project-selector");
  desktop
}

/// The id of the switcher's row of `name`.
fn switcher_row(desktop: &mut Desktop, name: &str) -> String {
  (0..5)
    .map(|index| format!("switcher-project-{index}"))
    .find(|id| {
      desktop
        .tree
        .get_element_by_id(id)
        .is_some_and(|row| contains_text(row, name))
    })
    .unwrap_or_else(|| panic!("the switcher lists {name}"))
}

fn selector_shows(desktop: &mut Desktop, name: &str) -> bool {
  desktop
    .tree
    .get_element_by_id("project-selector")
    .is_some_and(|selector| contains_text(selector, name))
}

#[test]
fn tabs_new_tab_opens_all_projects_and_closing_it_returns() {
  let mut desktop = kontur();
  assert_eq!(tab_ids(&mut desktop), [1]);
  assert!(shows(&mut desktop, "window-tab-1", "Kontur · Overview"));
  desktop.press("window-new-tab");
  desktop.wait_for("all-projects");
  assert_eq!(tab_ids(&mut desktop), [1, 2]);
  assert_eq!(active_tab(&mut desktop), Some(2));
  assert!(shows(&mut desktop, "window-tab-2", "All projects"));
  assert!(
    !has(&mut desktop, "project-selector"),
    "the sidebar follows the active tab"
  );
  desktop.press("window-tab-close-2");
  desktop.wait_for("nav-tasks");
  assert_eq!(tab_ids(&mut desktop), [1]);
  assert_eq!(desktop.focused(), None, "tabs are pointer-only");
}

#[test]
fn tabs_closing_the_last_tab_opens_all_projects() {
  let mut desktop = kontur();
  desktop.press("window-tab-close-1");
  desktop.wait_for("all-projects");
  let ids = tab_ids(&mut desktop);
  assert_eq!(ids.len(), 1, "the window keeps one tab");
  assert_ne!(ids[0], 1, "a fresh tab");
  let home = ids[0];
  assert!(
    !has(&mut desktop, &format!("window-tab-close-{home}")),
    "a sole All projects tab has no close button"
  );
  command(&mut desktop, "w", "KeyW", false);
  assert_eq!(tab_ids(&mut desktop), [home], "Ctrl+W does nothing there");
  assert!(has(&mut desktop, "all-projects"));
}

#[test]
fn tabs_shortcuts_open_switch_and_close() {
  let mut desktop = kontur();
  command(&mut desktop, "t", "KeyT", false);
  desktop.wait_for("all-projects");
  command(&mut desktop, "t", "KeyT", false);
  assert_eq!(tab_ids(&mut desktop), [1, 2, 3]);
  assert_eq!(active_tab(&mut desktop), Some(3));
  command(&mut desktop, "1", "Digit1", false);
  desktop.wait_for("nav-tasks");
  assert_eq!(active_tab(&mut desktop), Some(1));
  command(&mut desktop, "Tab", "Tab", true);
  assert_eq!(active_tab(&mut desktop), Some(3), "previous wraps to the last tab");
  command(&mut desktop, "Tab", "Tab", false);
  assert_eq!(active_tab(&mut desktop), Some(1), "next wraps to the first tab");
  command(&mut desktop, "9", "Digit9", false);
  assert_eq!(active_tab(&mut desktop), Some(3));
  if !cfg!(target_os = "macos") {
    command(&mut desktop, "PageUp", "PageUp", false);
    assert_eq!(active_tab(&mut desktop), Some(2));
  }
  let closing = active_tab(&mut desktop).expect("an active tab");
  command(&mut desktop, "w", "KeyW", false);
  assert!(
    !tab_ids(&mut desktop).contains(&closing),
    "Ctrl+W closes the active tab"
  );
  assert_eq!(tab_ids(&mut desktop).len(), 2);
}

#[test]
fn tabs_middle_click_closes_a_tab_without_switching() {
  let mut desktop = kontur();
  desktop.press("window-new-tab");
  desktop.wait_for("all-projects");
  command(&mut desktop, "1", "Digit1", false);
  desktop.wait_for("nav-tasks");
  click_with(&mut desktop, "window-tab-2", MouseButton::Middle, (false, false));
  assert_eq!(tab_ids(&mut desktop), [1]);
  assert_eq!(active_tab(&mut desktop), Some(1));
}

#[test]
fn tabs_switcher_opens_projects_in_new_tabs() {
  let mut desktop = two_projects();
  desktop.press("project-selector");
  desktop.wait_for("switcher-project-1");
  click_with(&mut desktop, "switcher-project-1", MouseButton::Left, (true, false));
  desktop.settle("a background tab", |tree| {
    tree.get_element_by_id("window-tab-2").is_some()
  });
  assert_eq!(active_tab(&mut desktop), Some(1), "Ctrl+click does not switch");
  assert!(selector_shows(&mut desktop, "Kontur"));
  desktop.settle("its label", |tree| {
    tree
      .get_element_by_id("window-tab-2")
      .is_some_and(|tab| contains_text(tab, "Personal website · Overview"))
  });

  // Opening Personal website made it the most recent project, the first row.
  desktop.press("project-selector");
  desktop.wait_for("switcher-project-1");
  assert!(shows(&mut desktop, "switcher-project-0", "Personal website"));
  click_with(&mut desktop, "switcher-project-0", MouseButton::Left, (true, true));
  desktop.settle("an active new tab", |tree| {
    tree.get_element_by_id("window-tab-3").is_some()
  });
  desktop.settle("Personal website shown", |tree| {
    tree
      .get_element_by_id("project-selector")
      .is_some_and(|selector| contains_text(selector, "Personal website"))
  });
  assert_eq!(active_tab(&mut desktop), Some(3), "Ctrl+Shift+click switches");
  assert_eq!(
    tab_ids(&mut desktop),
    [1, 3, 2],
    "new tabs open right of the current one"
  );
}

#[test]
fn tabs_switcher_row_offers_open_in_new_tab() {
  let mut desktop = two_projects();
  desktop.press("project-selector");
  desktop.wait_for("switcher-project-1");
  assert!(
    !has(&mut desktop, "switcher-new-tab-1"),
    "only the hovered row shows it"
  );
  desktop.hover("switcher-project-1");
  desktop.wait_for("switcher-new-tab-1");
  desktop.hover("switcher-new-tab-1");
  desktop.wait_for("switcher-new-tab-tooltip");
  assert!(shows(&mut desktop, "switcher-new-tab-tooltip", "Open in new tab"));
  desktop.press("switcher-new-tab-1");
  desktop.settle("a background tab", |tree| {
    tree.get_element_by_id("window-tab-2").is_some()
  });
  assert_eq!(active_tab(&mut desktop), Some(1));
  // A middle click on a row opens it in a new tab too, the shown project included.
  desktop.press("project-selector");
  desktop.wait_for("switcher-project-1");
  let kontur = switcher_row(&mut desktop, "Kontur");
  click_with(&mut desktop, &kontur, MouseButton::Middle, (false, false));
  desktop.settle("a second Kontur tab", |tree| {
    tree.get_element_by_id("window-tab-3").is_some()
  });
  assert_eq!(active_tab(&mut desktop), Some(1));
  assert!(shows(&mut desktop, "window-tab-3", "Kontur · Overview"));
}

#[test]
fn tabs_all_projects_rows_open_here_or_in_a_new_tab() {
  let mut desktop = two_projects();
  command(&mut desktop, "t", "KeyT", false);
  desktop.wait_for("project-row-1");
  click_with(&mut desktop, "project-row-1", MouseButton::Left, (true, false));
  desktop.settle("a background tab", |tree| {
    tree.get_element_by_id("window-tab-3").is_some()
  });
  assert_eq!(active_tab(&mut desktop), Some(2));
  assert!(has(&mut desktop, "all-projects"), "All projects stays in its tab");
  desktop.press("project-row-1");
  desktop.settle("Personal website in this tab", |tree| {
    tree
      .get_element_by_id("project-selector")
      .is_some_and(|selector| contains_text(selector, "Personal website"))
  });
  assert_eq!(
    active_tab(&mut desktop),
    Some(2),
    "the All projects tab became the project's tab"
  );
  assert!(shows(&mut desktop, "window-tab-2", "Personal website · Overview"));
}

#[test]
fn tabs_back_and_forward_act_on_the_active_tab() {
  let mut desktop = kontur();
  desktop.press("nav-tasks");
  desktop.wait_for("tasks-new-task");
  desktop.press("window-new-tab");
  desktop.wait_for("all-projects");
  // The new tab has no history before All projects.
  side_button(&mut desktop, BACK);
  assert!(has(&mut desktop, "all-projects"));
  command(&mut desktop, "1", "Digit1", false);
  desktop.wait_for("tasks-new-task");
  side_button(&mut desktop, BACK);
  desktop.wait_for("new-task");
  assert!(!has(&mut desktop, "tasks-new-task"), "tab 1 went back to its overview");
  command(&mut desktop, "2", "Digit2", false);
  desktop.wait_for("all-projects");
}

#[test]
fn tabs_inactive_tab_keeps_its_draft_and_scroll() {
  let mut desktop = kontur();
  desktop.press("nav-tasks");
  desktop.wait_for("tasks-new-task");
  desktop.click("tasks-new-task");
  desktop.wait_for("task-title");
  desktop.tab_to("task-title", 30);
  desktop.type_text("Half written");
  command(&mut desktop, "t", "KeyT", false);
  desktop.wait_for("all-projects");
  assert!(!has(&mut desktop, "task-title"), "an inactive tab is not laid out");
  command(&mut desktop, "1", "Digit1", false);
  desktop.wait_for("task-title");
  assert_eq!(desktop.value("task-title"), "Half written");

  desktop.click("nav-settings");
  desktop.wait_for("settings-repository");
  desktop.tree.resize(1440, 560);
  desktop.pass();
  let before = desktop.bounds("settings-repository").y;
  let area = desktop.bounds("settings-general");
  let (x, y) = (area.x + area.width / 2.0, area.y + area.height / 2.0);
  desktop.tree.scroll(x, y, 0.0, 0.0, ScrollPhase::Start);
  desktop.tree.scroll(x, y, 0.0, -400.0, ScrollPhase::Scroll);
  desktop.tree.scroll(x, y, 0.0, 0.0, ScrollPhase::End);
  desktop.pass();
  let scrolled = desktop.bounds("settings-repository").y;
  assert!(scrolled < before, "the settings scrolled: {scrolled} from {before}");
  command(&mut desktop, "2", "Digit2", false);
  desktop.wait_for("all-projects");
  command(&mut desktop, "1", "Digit1", false);
  desktop.wait_for("settings-repository");
  let back = desktop.bounds("settings-repository").y;
  assert!(
    (back - scrolled).abs() < 0.5,
    "the scroll is kept: {back} instead of {scrolled}"
  );
}

#[test]
fn tabs_inactive_new_task_keeps_its_scroll() {
  let mut desktop = kontur();
  desktop.press("nav-tasks");
  desktop.wait_for("tasks-new-task");
  desktop.click("tasks-new-task");
  desktop.wait_for("new-task-brief");
  desktop.tree.resize(1440, 560);
  desktop.pass();

  let before = desktop.bounds("task-title").y;
  let area = desktop.bounds("new-task-brief");
  let (x, y) = (area.x + area.width / 2.0, area.y + area.height / 2.0);
  desktop.tree.scroll(x, y, 0.0, 0.0, ScrollPhase::Start);
  desktop.tree.scroll(x, y, 0.0, -400.0, ScrollPhase::Scroll);
  desktop.tree.scroll(x, y, 0.0, 0.0, ScrollPhase::End);
  desktop.pass();
  let scrolled = desktop.bounds("task-title").y;
  assert!(
    scrolled < before,
    "the New Task form scrolled: {scrolled} from {before}"
  );

  command(&mut desktop, "t", "KeyT", false);
  desktop.wait_for("all-projects");
  command(&mut desktop, "1", "Digit1", false);
  desktop.wait_for("new-task-brief");
  let back = desktop.bounds("task-title").y;
  assert!(
    (back - scrolled).abs() < 0.5,
    "the New Task scroll is kept: {back} instead of {scrolled}"
  );
}

#[test]
fn tabs_closing_a_tab_with_unsaved_edits_asks_first() {
  let mut desktop = kontur();
  desktop.click("nav-tools");
  desktop.wait_for("tools-empty-add");
  desktop.click("tools-empty-add");
  desktop.wait_for("tool-name");
  desktop.tab_to("tool-name", 30);
  desktop.type_text("Checks");
  command(&mut desktop, "w", "KeyW", false);
  desktop.wait_for("discard-changes-dialog");
  assert!(shows(&mut desktop, "discard-changes-dialog", "Discard changes?"));
  desktop.press("discard-changes-keep");
  assert!(!has(&mut desktop, "discard-changes-dialog"));
  assert_eq!(tab_ids(&mut desktop), [1], "Keep open keeps the tab");
  assert_eq!(desktop.value("tool-name"), "Checks");

  desktop.press("window-tab-close-1");
  desktop.wait_for("discard-changes-dialog");
  desktop.key("Escape", "Escape");
  assert!(!has(&mut desktop, "discard-changes-dialog"), "Escape keeps it open");
  assert_eq!(tab_ids(&mut desktop), [1]);

  command(&mut desktop, "w", "KeyW", false);
  desktop.wait_for("discard-changes-confirm");
  desktop.press("discard-changes-confirm");
  desktop.wait_for("all-projects");
  assert!(!tab_ids(&mut desktop).contains(&1), "Discard and close closes it");
}

#[test]
fn tabs_a_page_without_edits_closes_without_asking() {
  let mut desktop = kontur();
  desktop.press("window-new-tab");
  desktop.wait_for("all-projects");
  command(&mut desktop, "1", "Digit1", false);
  desktop.wait_for("nav-tasks");
  command(&mut desktop, "w", "KeyW", false);
  assert!(!has(&mut desktop, "discard-changes-dialog"));
  assert_eq!(tab_ids(&mut desktop), [2]);
}

#[test]
fn tabs_do_not_react_under_a_dialog() {
  let mut desktop = kontur();
  desktop.press("window-new-tab");
  desktop.wait_for("all-projects");
  command(&mut desktop, "1", "Digit1", false);
  desktop.wait_for("nav-pipelines");
  desktop.click("nav-pipelines");
  desktop.wait_for("pipelines-new");
  desktop.click("pipelines-new");
  desktop.wait_for("new-pipeline-name");
  for press in ["window-new-tab", "window-tab-2", "window-tab-close-1"] {
    if has(&mut desktop, press) {
      desktop.press(press);
    }
  }
  command(&mut desktop, "t", "KeyT", false);
  command(&mut desktop, "Tab", "Tab", false);
  command(&mut desktop, "w", "KeyW", false);
  assert_eq!(tab_ids(&mut desktop), [1, 2], "nothing opened or closed");
  assert_eq!(active_tab(&mut desktop), Some(1), "nothing switched");
  assert!(has(&mut desktop, "new-pipeline-dialog"), "the dialog stays open");
}

/// Hovering a tab hides the dividers beside it; the strip keeps its elements in place, so a
/// press right after the pointer arrived still activates the tab it lands on.
#[test]
fn tabs_a_click_right_after_the_pointer_arrives_activates_that_tab() {
  let mut desktop = kontur();
  command(&mut desktop, "t", "KeyT", false);
  command(&mut desktop, "t", "KeyT", false);
  command(&mut desktop, "1", "Digit1", false);
  desktop.wait_for("nav-tasks");
  let bounds = desktop.bounds("window-tab-3");
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  desktop.tree.mouse_move(x, y);
  desktop.tree.mouse_down(x, y, MouseButton::Left);
  desktop.pass();
  desktop.tree.mouse_up(x, y, MouseButton::Left);
  desktop.pass();
  assert_eq!(active_tab(&mut desktop), Some(3));
}
