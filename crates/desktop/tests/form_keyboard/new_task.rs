use crate::{
  github::shows,
  harness::{Desktop, expected},
};

const TITLE: &str = "task-title";
const BRIEF: &str = "task-brief";
const FIRST_CRITERION: &str = "criterion-0";
const SECOND_CRITERION: &str = "criterion-1";
const ADD_CRITERION: &str = "add-criterion";
const BUDGET: &str = "task-budget";
const TIME_LIMIT: &str = "task-time-limit";
/// More than the window has stops on this screen.
const MAX_STOPS: usize = 30;

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
fn new_task_tab_passes_through_the_form_in_window_order() {
  let mut desktop = new_task();
  // Only the form: the sidebar, back and the status bar are pointer-only. Attach context, the
  // pipeline selector without pipelines, Create & run and Save as draft (no title yet) cannot
  // be used and are skipped. After the form Tab wraps to its first field.
  assert_eq!(
    desktop.stops(7, false),
    expected(&[
      Some(TITLE),
      Some(BRIEF),
      Some(FIRST_CRITERION),
      Some(ADD_CRITERION),
      Some(BUDGET),
      Some(TIME_LIMIT),
      Some(TITLE),
    ])
  );
  assert_eq!(desktop.stops(2, true), expected(&[Some(TIME_LIMIT), Some(BUDGET)]));
}

#[test]
fn new_task_criteria_are_added_and_removed_from_the_keyboard() {
  let mut desktop = new_task();
  desktop.tab_to(ADD_CRITERION, MAX_STOPS);
  desktop.enter();
  desktop.wait_for(SECOND_CRITERION);
  // Each criterion now has its remove button after it.
  assert_eq!(
    desktop.stops(5, true),
    expected(&[
      Some("remove-criterion-1"),
      Some(SECOND_CRITERION),
      Some("remove-criterion-0"),
      Some(FIRST_CRITERION),
      Some(BRIEF),
    ])
  );
  assert_eq!(
    desktop.stops(2, false),
    expected(&[Some(FIRST_CRITERION), Some("remove-criterion-0")])
  );
  desktop.key(" ", "Space");
  desktop.settle("the first criterion to go", |tree| {
    tree.get_element_by_id(FIRST_CRITERION).is_none()
  });
  assert!(desktop.tree.get_element_by_id(SECOND_CRITERION).is_some());
}

#[test]
fn new_task_enter_in_the_brief_adds_a_line() {
  let mut desktop = new_task();
  desktop.tab_to(BRIEF, MAX_STOPS);
  desktop.type_text("a");
  desktop.enter();
  desktop.type_text("b");
  assert_eq!(desktop.value(BRIEF), "a\nb");
  assert!(desktop.tree.get_element_by_id(TITLE).is_some());
}

#[test]
fn new_task_enter_saves_only_a_valid_draft() {
  let mut desktop = new_task();
  desktop.tab_to(TITLE, MAX_STOPS);
  desktop.type_text("Add switcher");
  desktop.tab_to(BUDGET, MAX_STOPS);
  desktop.type_text("x");
  assert_eq!(desktop.value(BUDGET), "1,400,000x");
  // The budget is not a number: Save as draft is disabled and Enter saves nothing.
  desktop.enter();
  desktop.settle("the form to stay", |tree| tree.get_element_by_id(TITLE).is_some());
  desktop.key("Backspace", "Backspace");
  desktop.tab_to(TITLE, MAX_STOPS);
  desktop.enter();
  desktop.settle("the saved draft in the task list", |tree| {
    tree.get_element_by_id(TITLE).is_none() && tree.get_element_by_id("tasks-new-task").is_some()
  });
}

#[test]
fn new_task_budget_starts_at_the_project_default_and_groups_digits_while_typing() {
  let mut desktop = new_task();
  desktop.settle("the default budget", |tree| {
    tree
      .get_element_by_id_mut(BUDGET)
      .and_then(|element| element.as_text_input())
      .is_some_and(|input| input.value() == "1,400,000")
  });
  assert_eq!(desktop.value(TIME_LIMIT), "30");
  assert!(shows(
    &mut desktop,
    "task-budget-readback",
    "= 1.4M tokens · input + output · max 10M per run"
  ));
  assert!(shows(
    &mut desktop,
    "task-time-readback",
    "Active agent time · pauses and waiting for you don’t count"
  ));
  desktop.tab_to(BUDGET, MAX_STOPS);
  for _ in 0.."1,400,000".len() {
    desktop.key("Backspace", "Backspace");
  }
  assert!(shows(&mut desktop, "execution-plan", "Enter a token budget."));
  desktop.type_text("25000000");
  assert_eq!(desktop.value(BUDGET), "25,000,000");
  assert!(shows(
    &mut desktop,
    "execution-plan",
    "A run budget can be at most 10M tokens (project setting)."
  ));
  desktop.key("Backspace", "Backspace");
  assert_eq!(desktop.value(BUDGET), "2,500,000");
  assert!(shows(&mut desktop, "task-budget-readback", "= 2.5M tokens"));
  desktop.type_text("k");
  assert_eq!(desktop.value(BUDGET), "2,500,000k");
  assert!(shows(
    &mut desktop,
    "execution-plan",
    "Use digits only, for example 1,400,000."
  ));
}
