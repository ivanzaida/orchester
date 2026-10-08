//! Edit task's *Add blocking task* search (`tUwII`) as a person uses it: an option is chosen with a
//! click on it, or by Tab to it and Enter, and *Add selected task* then adds it. The options are
//! form controls, Tab stops pressed with Enter or Space; the search does not move through them with
//! the arrow keys, like every option list of the app.

use lurq::app::events::ScrollPhase;

use crate::{
  harness::Desktop,
  organization::{seed_dependencies, tasks},
};

/// The scrolling column of the task form's fields.
const COLUMN: &str = "new-task-brief";
/// A wheel turn; a negative delta scrolls the column down.
const WHEEL: f32 = 120.0;
const WHEEL_TURNS: usize = 30;
/// Into the column's scroll gutter from its right edge, where a wheel turn scrolls the column and
/// no field inside it.
const GUTTER: f32 = 6.0;

/// KON-1 (KON-2 is blocked by it, KON-3 is free) open in Edit task.
fn edit_first_task() -> Desktop {
  let mut desktop = Desktop::with_seeded_project(seed_dependencies);
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  desktop.press("task-row-KON-1");
  desktop.wait_for("task-edit");
  desktop.click("task-edit");
  desktop.wait_for("task-blocked-field");
  desktop
}

/// Wheels the column until `id` is wholly in view, as a person scrolls to what they mean to click.
fn scroll_to(desktop: &mut Desktop, id: &str) {
  for _ in 0..WHEEL_TURNS {
    let (column, target) = (desktop.bounds(COLUMN), desktop.bounds(id));
    if target.y >= column.y && target.y + target.height <= column.y + column.height {
      return;
    }
    let delta = if target.y < column.y { WHEEL } else { -WHEEL };
    let (x, y) = (column.x + column.width - GUTTER, column.y + column.height / 2.0);
    desktop.tree.scroll(x, y, 0.0, 0.0, ScrollPhase::Start);
    desktop.tree.scroll(x, y, 0.0, delta, ScrollPhase::Scroll);
    desktop.tree.scroll(x, y, 0.0, 0.0, ScrollPhase::End);
    desktop.pass();
  }
  panic!("{id} does not scroll into view");
}

/// The pointer moves onto `id` in view and clicks it.
fn click_in_view(desktop: &mut Desktop, id: &str) {
  desktop.wait_for(id);
  scroll_to(desktop, id);
  desktop.hover(id);
  desktop.press(id);
}

/// Waits until the form shows the link to KON-3 (Run names it as unfinished) and the search has
/// closed, then saves and checks that KON-1 is stored blocked by KON-3.
fn assert_kon_3_linked_and_saved(desktop: &mut Desktop) {
  desktop.wait_for("task-run-unavailable");
  assert!(crate::github::shows(
    desktop,
    "task-run-unavailable",
    "Blocked by KON-3"
  ));
  assert!(desktop.tree.get_element_by_id("blocking-task-search").is_none());
  assert!(desktop.tree.get_element_by_id("task-open-blocking-picker").is_some());
  desktop.tab_to("save-draft", 40);
  desktop.enter();
  desktop.wait_until("the link saved", |desktop| {
    tasks(&desktop.repo)[0].organization().dependencies().len() == 1
  });
  let stored = tasks(&desktop.repo);
  assert_eq!(stored[0].organization().dependencies()[0].task, stored[2].id());
}

#[test]
fn organization_dependency_picker_adds_a_task_chosen_with_the_pointer() {
  let mut desktop = edit_first_task();
  click_in_view(&mut desktop, "task-blocked-field");
  click_in_view(&mut desktop, "task-blocked-reason");
  desktop.type_text("Waiting");
  click_in_view(&mut desktop, "task-open-blocking-picker");
  click_in_view(&mut desktop, "blocking-task-search");
  desktop.type_text("kon-");
  desktop.wait_for("task-blocking-option-KON-3");
  // Nothing is chosen yet: Add selected task adds nothing and the search stays open.
  click_in_view(&mut desktop, "task-add-blocking-link");
  assert!(desktop.tree.get_element_by_id("blocking-task-search").is_some());
  click_in_view(&mut desktop, "task-blocking-option-KON-3");
  click_in_view(&mut desktop, "task-add-blocking-link");
  assert_kon_3_linked_and_saved(&mut desktop);
}

#[test]
fn organization_dependency_picker_adds_a_task_chosen_with_the_keyboard() {
  let mut desktop = edit_first_task();
  desktop.tab_to("task-blocked-field", 40);
  desktop.enter();
  desktop.wait_for("task-blocked-reason");
  desktop.tab_to("task-blocked-reason", 40);
  desktop.type_text("Waiting");
  desktop.tab_to("task-open-blocking-picker", 40);
  desktop.enter();
  desktop.wait_for("blocking-task-search");
  desktop.tab_to("blocking-task-search", 40);
  desktop.type_text("kon-");
  desktop.wait_for("task-blocking-option-KON-3");
  desktop.tab_to("task-blocking-option-KON-3", 40);
  desktop.enter();
  desktop.tab_to("task-add-blocking-link", 40);
  desktop.enter();
  assert_kon_3_linked_and_saved(&mut desktop);
}
