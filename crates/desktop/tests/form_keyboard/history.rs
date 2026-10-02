//! Back and forward: the mouse's side buttons (on release) and the history shortcuts move
//! through the screens the user opened, tabs included. A submitted form (Create, Create & run) is
//! not visited again, no screen follows itself in history, and Alt+Left in a focused text field
//! stays with the field.

use lurq::app::{Tree, events::MouseButton};

use crate::{harness::Desktop, new_task::highlighted};

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

/// Types `title` into the open New task form and presses Enter, which is Create.
fn create(desktop: &mut Desktop, title: &str) {
  desktop.tab_to(TITLE, MAX_STOPS);
  desktop.type_text(title);
  desktop.enter();
}

/// The draft "Add switcher", made with Create from Tasks, which the form returned to.
fn saved_draft() -> Desktop {
  let mut desktop = Desktop::with_project();
  desktop.wait_for(OVERVIEW);
  desktop.press("nav-tasks");
  desktop.wait_for(TASKS);
  desktop.click(TASKS);
  desktop.wait_for(TITLE);
  create(&mut desktop, "Add switcher");
  desktop.settle("the new task in the list", |tree| {
    tree.get_element_by_id(TITLE).is_none() && highlighted(tree, "KON-1")
  });
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

/// Whether the tab `id` of the task list is the selected one: a selected tab, like one that
/// cannot be opened yet, is not a button.
fn tab_selected(tree: &mut Tree, id: &str) -> bool {
  let unavailable = tree
    .get_element_by_id("tasks-tab-queue")
    .map(|tab| tab.tag_name().to_owned());
  tree
    .get_element_by_id(id)
    .is_some_and(|tab| Some(tab.tag_name()) == unavailable.as_deref())
}

#[test]
fn history_create_returns_to_the_list_and_is_not_visited_again() {
  let mut desktop = saved_draft();
  // Create went back to the list the form was opened from: going back leads to the overview
  // before it, and forward past the list stays there instead of reopening the form.
  side_button(&mut desktop, BACK);
  desktop.wait_for(OVERVIEW);
  side_button(&mut desktop, FORWARD);
  desktop.wait_for("task-row-KON-1");
  side_button(&mut desktop, FORWARD);
  desktop.pass();
  assert!(shows(&mut desktop, "task-row-KON-1"));
  assert!(!shows(&mut desktop, TITLE));
}

#[test]
fn history_back_restores_the_tab() {
  let mut desktop = saved_draft();
  assert!(tab_selected(&mut desktop.tree, "tasks-tab-all"));
  assert!(!tab_selected(&mut desktop.tree, "tasks-tab-drafts"));
  desktop.press("tasks-tab-drafts");
  desktop.settle("the Drafts tab", |tree| tab_selected(tree, "tasks-tab-drafts"));
  desktop.press("task-row-KON-1");
  desktop.wait_for(TASK_PAGE);
  side_button(&mut desktop, BACK);
  desktop.settle("the list on the Drafts tab", |tree| {
    tree.get_element_by_id(TASK_PAGE).is_none() && tab_selected(tree, "tasks-tab-drafts")
  });
  side_button(&mut desktop, BACK);
  desktop.settle("the All tab", |tree| {
    tab_selected(tree, "tasks-tab-all") && !tab_selected(tree, "tasks-tab-drafts")
  });
}

#[test]
fn history_saved_draft_returns_to_the_tab_it_was_written_from() {
  let mut desktop = saved_draft();
  desktop.press("tasks-tab-drafts");
  desktop.settle("the Drafts tab", |tree| tab_selected(tree, "tasks-tab-drafts"));
  desktop.click(TASKS);
  desktop.wait_for(TITLE);
  create(&mut desktop, "Second draft");
  desktop.settle("the second draft on the Drafts tab", |tree| {
    highlighted(tree, "KON-2") && tab_selected(tree, "tasks-tab-drafts")
  });
  // The Drafts tab is one entry: going back leads to the All tab before it.
  side_button(&mut desktop, BACK);
  desktop.settle("the All tab", |tree| tab_selected(tree, "tasks-tab-all"));
}

/// The form's own back arrow returns to the list it was opened from, on its tab, instead of
/// opening the list again after the form.
#[test]
fn history_new_task_back_arrow_returns_to_the_list_and_its_tab() {
  let mut desktop = saved_draft();
  desktop.press("tasks-tab-drafts");
  desktop.settle("the Drafts tab", |tree| tab_selected(tree, "tasks-tab-drafts"));
  desktop.click(TASKS);
  desktop.wait_for(TITLE);
  desktop.press("new-task-back");
  desktop.settle("the list on the Drafts tab", |tree| {
    tree.get_element_by_id(TITLE).is_none() && tab_selected(tree, "tasks-tab-drafts")
  });
  // The list was not opened again: going back leads to the All tab before it, and forward to
  // the form that was left.
  side_button(&mut desktop, BACK);
  desktop.settle("the All tab", |tree| tab_selected(tree, "tasks-tab-all"));
  side_button(&mut desktop, FORWARD);
  desktop.settle("the Drafts tab", |tree| tab_selected(tree, "tasks-tab-drafts"));
  side_button(&mut desktop, FORWARD);
  desktop.wait_for(TITLE);
}

/// Opened from another screen, the back arrow opens the task list as a new entry.
#[test]
fn history_new_task_back_arrow_from_the_overview_opens_the_list() {
  let mut desktop = Desktop::with_project();
  desktop.wait_for(OVERVIEW);
  desktop.click(OVERVIEW);
  desktop.wait_for(TITLE);
  desktop.press("new-task-back");
  desktop.wait_for(TASKS);
  side_button(&mut desktop, BACK);
  desktop.wait_for(TITLE);
  side_button(&mut desktop, BACK);
  desktop.wait_for(OVERVIEW);
}

/// Opened from the overview, Create puts the task list in the form's place.
#[test]
fn history_create_from_the_overview_takes_the_form_s_place() {
  let mut desktop = Desktop::with_project();
  desktop.wait_for(OVERVIEW);
  desktop.click(OVERVIEW);
  desktop.wait_for(TITLE);
  create(&mut desktop, "Add switcher");
  desktop.settle("the new task in the list", |tree| {
    tree.get_element_by_id(TITLE).is_none() && highlighted(tree, "KON-1")
  });
  side_button(&mut desktop, BACK);
  desktop.wait_for(OVERVIEW);
  assert!(!shows(&mut desktop, TITLE));
  side_button(&mut desktop, FORWARD);
  desktop.wait_for("task-row-KON-1");
}

#[test]
fn history_overview_tab_is_restored() {
  let mut desktop = saved_draft();
  desktop.press("nav-overview");
  desktop.wait_for("overview-tab-running");
  desktop.press("overview-tab-running");
  // A draft is not running: the Running tab lists no task.
  desktop.settle("the Running tab", |tree| {
    tree.get_element_by_id("task-row-KON-1").is_none()
  });
  desktop.press("nav-tasks");
  desktop.wait_for(TASKS);
  side_button(&mut desktop, BACK);
  desktop.settle("the overview on the Running tab", |tree| {
    tree.get_element_by_id("overview-tab-running").is_some() && tree.get_element_by_id("task-row-KON-1").is_none()
  });
  side_button(&mut desktop, BACK);
  desktop.wait_for("task-row-KON-1");
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
  desktop.wait_for("settings-limits");
  side_button(&mut desktop, BACK);
  desktop.settle("the General tab", |tree| {
    tree.get_element_by_id("settings-limits").is_none() && tree.get_element_by_id("settings-general").is_some()
  });
  side_button(&mut desktop, FORWARD);
  desktop.wait_for("settings-limits");
}

#[test]
fn history_create_and_run_leaves_one_entry_per_screen() {
  use crate::task_run::{create_and_run, init_repository, programs, seed};

  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  create_and_run(&mut desktop, "Append one line to README.md.", "1400000");
  // The task page took the form's place after the task list it was opened from.
  side_button(&mut desktop, BACK);
  desktop.settle("the task list", |tree| {
    tree.get_element_by_id(TASK_PAGE).is_none()
      && tree.get_element_by_id(TITLE).is_none()
      && tree.get_element_by_id(TASKS).is_some()
  });
  side_button(&mut desktop, BACK);
  desktop.wait_for(OVERVIEW);
}
