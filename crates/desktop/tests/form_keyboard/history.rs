//! Back and forward: the mouse's side buttons (on release) and the history shortcuts move
//! through the screens the user opened. A submitted form is not visited again, and Alt+Left in
//! a focused text field stays with the field.

use lurq::app::events::MouseButton;

use crate::harness::Desktop;

const OVERVIEW: &str = "new-task";
const TASKS: &str = "tasks-new-task";
const PIPELINES: &str = "pipelines-new";
const TASK_PAGE: &str = "task-page";
const TITLE: &str = "task-title";
const BACK: MouseButton = MouseButton::Other(4);
const FORWARD: MouseButton = MouseButton::Other(5);
/// More than the window has stops on the new task form.
const MAX_STOPS: usize = 30;

/// The overview, then Tasks and Pipelines opened from the sidebar.
fn three_screens() -> Desktop {
  let mut desktop = Desktop::with_project();
  desktop.wait_for(OVERVIEW);
  desktop.press("nav-tasks");
  desktop.wait_for(TASKS);
  desktop.press("nav-pipelines");
  desktop.wait_for(PIPELINES);
  desktop
}

fn shows(desktop: &mut Desktop, id: &str) -> bool {
  desktop.tree.get_element_by_id(id).is_some()
}

/// A side button pressed and released over the middle of the window.
fn side_button(desktop: &mut Desktop, button: MouseButton) {
  desktop.tree.mouse_down(720.0, 480.0, button);
  desktop.pass();
  desktop.tree.mouse_up(720.0, 480.0, button);
  desktop.pass();
}

#[cfg(not(target_os = "macos"))]
fn shortcut(desktop: &mut Desktop, back: bool) {
  let key = if back { "ArrowLeft" } else { "ArrowRight" };
  desktop.tree.key_down(key.into(), key.into(), false, false, true);
  desktop.pass();
}

#[cfg(target_os = "macos")]
fn shortcut(desktop: &mut Desktop, back: bool) {
  let (key, code) = if back {
    ("[", "BracketLeft")
  } else {
    ("]", "BracketRight")
  };
  desktop
    .tree
    .key_down_with_meta(key.into(), code.into(), false, false, false, true);
  desktop.pass();
}

/// The saved draft "Add switcher", listed in Tasks after New task.
fn saved_draft() -> Desktop {
  let mut desktop = Desktop::with_project();
  desktop.wait_for(OVERVIEW);
  desktop.press("nav-tasks");
  desktop.wait_for(TASKS);
  desktop.click(TASKS);
  desktop.wait_for(TITLE);
  desktop.tab_to(TITLE, MAX_STOPS);
  desktop.type_text("Add switcher");
  desktop.enter();
  desktop.wait_for("task-row-KON-1");
  desktop
}

#[test]
fn history_side_buttons_go_back_and_forward_on_release() {
  let mut desktop = three_screens();
  desktop.tree.mouse_down(720.0, 480.0, BACK);
  desktop.pass();
  assert!(shows(&mut desktop, PIPELINES), "a press alone does not go back");
  desktop.tree.mouse_up(720.0, 480.0, BACK);
  desktop.wait_for(TASKS);
  side_button(&mut desktop, BACK);
  desktop.wait_for(OVERVIEW);
  // The first screen has nothing before it.
  side_button(&mut desktop, BACK);
  assert!(shows(&mut desktop, OVERVIEW));
  side_button(&mut desktop, FORWARD);
  desktop.wait_for(TASKS);
  side_button(&mut desktop, FORWARD);
  desktop.wait_for(PIPELINES);
}

#[test]
fn history_shortcuts_go_back_and_forward() {
  let mut desktop = three_screens();
  shortcut(&mut desktop, true);
  desktop.wait_for(TASKS);
  shortcut(&mut desktop, true);
  desktop.wait_for(OVERVIEW);
  shortcut(&mut desktop, false);
  desktop.wait_for(TASKS);
}

#[cfg(not(target_os = "macos"))]
#[test]
fn history_alt_left_stays_with_a_focused_text_field() {
  let mut desktop = Desktop::with_project();
  desktop.wait_for(OVERVIEW);
  desktop.press("nav-tasks");
  desktop.wait_for(TASKS);
  desktop.click(TASKS);
  desktop.wait_for(TITLE);
  desktop.tab_to(TITLE, MAX_STOPS);
  desktop.type_text("ab");
  shortcut(&mut desktop, true);
  assert!(shows(&mut desktop, TITLE), "the form is still shown");
  assert_eq!(desktop.focused().as_deref(), Some(TITLE));
}

#[test]
fn history_submitted_form_is_not_visited_again() {
  let mut desktop = saved_draft();
  // The form's entry now holds the task list, so going back leaves the form out.
  side_button(&mut desktop, BACK);
  desktop.settle("the task list before the form", |tree| {
    tree.get_element_by_id(TASKS).is_some() && tree.get_element_by_id(TITLE).is_none()
  });
  side_button(&mut desktop, BACK);
  desktop.wait_for(OVERVIEW);
  side_button(&mut desktop, FORWARD);
  side_button(&mut desktop, FORWARD);
  desktop.wait_for("task-row-KON-1");
  assert!(!shows(&mut desktop, TITLE));
}

#[test]
fn history_task_row_opens_the_task_and_back_returns_to_the_list() {
  let mut desktop = saved_draft();
  desktop.press("task-row-KON-1");
  desktop.wait_for(TASK_PAGE);
  side_button(&mut desktop, BACK);
  desktop.settle("the task list", |tree| {
    tree.get_element_by_id(TASK_PAGE).is_none() && tree.get_element_by_id("task-row-KON-1").is_some()
  });
  side_button(&mut desktop, FORWARD);
  desktop.wait_for(TASK_PAGE);
}

#[test]
fn history_sidebar_item_leads_from_a_task_to_its_list() {
  let mut desktop = saved_draft();
  desktop.press("task-row-KON-1");
  desktop.wait_for(TASK_PAGE);
  // Tasks stays selected on the task page and still opens the list.
  desktop.press("nav-tasks");
  desktop.settle("the task list", |tree| {
    tree.get_element_by_id(TASK_PAGE).is_none() && tree.get_element_by_id(TASKS).is_some()
  });
  side_button(&mut desktop, BACK);
  desktop.wait_for(TASK_PAGE);
}

#[test]
fn history_a_tab_is_an_entry() {
  let mut desktop = Desktop::with_project();
  desktop.wait_for(OVERVIEW);
  desktop.press("nav-settings");
  desktop.wait_for("settings-tab-limits");
  desktop.press("settings-tab-limits");
  desktop.wait_for("settings-save-status");
  side_button(&mut desktop, BACK);
  desktop.settle("the General tab", |tree| {
    tree.get_element_by_id("settings-save-status").is_none() && tree.get_element_by_id("settings-tab-general").is_some()
  });
  side_button(&mut desktop, FORWARD);
  desktop.wait_for("settings-save-status");
}
