use crate::harness::{Desktop, expected};

const TITLE: &str = "task-title";
const BRIEF: &str = "task-brief";
const FIRST_CRITERION: &str = "criterion-0";
const SECOND_CRITERION: &str = "criterion-1";
const BUDGET: &str = "task-budget";
const TIME_LIMIT: &str = "task-time-limit";

fn new_task() -> Desktop {
  let mut desktop = Desktop::with_project();
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("tasks-new-task");
  desktop.click("tasks-new-task");
  desktop.wait_for(TITLE);
  desktop
}

#[test]
fn new_task_tab_follows_the_form_and_skips_what_cannot_be_used() {
  let mut desktop = new_task();
  let inputs = [TITLE, BRIEF, FIRST_CRITERION, BUDGET, TIME_LIMIT];
  // Attach context, the pipeline selector without pipelines, Create & run and Save as draft
  // (no title yet) cannot be used and are skipped; Add criterion is a stop.
  assert_eq!(
    desktop.stops(7, false, &inputs),
    expected(&[
      Some(TITLE),
      Some(BRIEF),
      Some(FIRST_CRITERION),
      None,
      Some(BUDGET),
      Some(TIME_LIMIT),
      Some(TITLE)
    ])
  );
  assert_eq!(
    desktop.stops(6, true, &inputs),
    expected(&[
      Some(TIME_LIMIT),
      Some(BUDGET),
      None,
      Some(FIRST_CRITERION),
      Some(BRIEF),
      Some(TITLE)
    ])
  );
}

#[test]
fn new_task_criteria_are_added_and_removed_from_the_keyboard() {
  let mut desktop = new_task();
  let inputs = [TITLE, BRIEF, FIRST_CRITERION, BUDGET];
  desktop.stops(4, false, &inputs);
  desktop.enter();
  desktop.wait_for(SECOND_CRITERION);
  // Each criterion now has its remove button after it.
  let inputs = [TITLE, BRIEF, FIRST_CRITERION, SECOND_CRITERION, BUDGET];
  assert_eq!(
    desktop.stops(5, true, &inputs),
    expected(&[None, Some(SECOND_CRITERION), None, Some(FIRST_CRITERION), Some(BRIEF)])
  );
  desktop.stops(2, false, &inputs);
  desktop.key(" ", "Space");
  desktop.settle("the first criterion to go", |tree| {
    tree.get_element_by_id(FIRST_CRITERION).is_none()
  });
  assert!(desktop.tree.get_element_by_id(SECOND_CRITERION).is_some());
}

#[test]
fn new_task_enter_in_the_brief_adds_a_line() {
  let mut desktop = new_task();
  desktop.stops(2, false, &[TITLE, BRIEF]);
  desktop.type_text("a");
  desktop.enter();
  desktop.type_text("b");
  assert_eq!(desktop.value(BRIEF), "a\nb");
  assert!(desktop.tree.get_element_by_id(TITLE).is_some());
}

#[test]
fn new_task_enter_saves_only_a_valid_draft() {
  let mut desktop = new_task();
  desktop.tab();
  desktop.type_text("Add switcher");
  let inputs = [TITLE, BRIEF, FIRST_CRITERION, BUDGET, TIME_LIMIT];
  assert_eq!(desktop.stops(4, false, &inputs).last(), Some(&Some(BUDGET.to_owned())));
  desktop.type_text("x");
  assert_eq!(desktop.value(BUDGET), "x");
  // The budget is not a number: Save as draft is disabled and Enter saves nothing.
  desktop.enter();
  desktop.settle("the form to stay", |tree| tree.get_element_by_id(TITLE).is_some());
  desktop.key("Backspace", "Backspace");
  assert_eq!(desktop.stops(4, true, &inputs).last(), Some(&Some(TITLE.to_owned())));
  desktop.enter();
  desktop.settle("the saved draft in the task list", |tree| {
    tree.get_element_by_id(TITLE).is_none() && tree.get_element_by_id("tasks-new-task").is_some()
  });
}
