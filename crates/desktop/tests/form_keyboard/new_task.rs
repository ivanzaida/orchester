use std::time::Duration;

use orchester_core::{
  id::{IdempotencyKey, ResourceId},
  name::Name,
  store::TaskStore,
  task::{TaskDetails, TaskLimits, TaskStatus},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_desktop::tasks::data::{SaveTarget, TaskDraftSaveError, save_draft, save_draft_in};
use orchester_store::ProjectDb;

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
  // The limits start at the project's defaults once those are read; a key typed before then
  // would keep the field from taking them.
  desktop.settle("the default budget", |tree| {
    tree
      .get_element_by_id_mut(BUDGET)
      .and_then(|element| element.as_text_input())
      .is_some_and(|input| input.value() == "1,400,000")
  });
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
  desktop.settle("the saved draft in the form", |tree| {
    tree
      .get_element_by_id("new-task-state")
      .and_then(|state| state.text_content())
      .is_some_and(|status| status.contains("Draft saved locally"))
  });
  assert!(desktop.tree.get_element_by_id(TITLE).is_some());
  let db = ProjectDb::open(&desktop.repo).expect("open project");
  assert_eq!(db.tasks().expect("read tasks").len(), 1);
}

#[test]
fn new_task_saved_state_tracks_edits_and_resave() {
  let mut desktop = new_task();
  desktop.tab_to(TITLE, MAX_STOPS);
  desktop.type_text("Add switcher");
  desktop.press("save-draft");
  desktop.settle("first draft save", |tree| {
    tree
      .get_element_by_id("new-task-state")
      .and_then(|state| state.text_content())
      .is_some_and(|status| status.contains("Draft saved locally"))
  });
  desktop.tab_to(BRIEF, MAX_STOPS);
  desktop.type_text("Add a switcher to the project.");
  assert!(shows(&mut desktop, "new-task-state", "Not saved yet"));
  desktop.press("save-draft");
  desktop.settle("edited draft save", |tree| {
    tree
      .get_element_by_id("new-task-state")
      .and_then(|state| state.text_content())
      .is_some_and(|status| status.contains("Draft saved locally"))
  });
  let db = ProjectDb::open(&desktop.repo).expect("open project");
  let tasks = db.tasks().expect("read tasks");
  assert_eq!(tasks.len(), 1);
  assert_eq!(tasks[0].details().brief, "Add a switcher to the project.");
}

#[test]
fn queued_task_cannot_be_reported_as_a_saved_draft() {
  let desktop = new_task();
  let batch = IdempotencyKey::generate();
  let details = TaskDetails {
    title: Name::new("Add switcher").expect("valid title"),
    brief: "Add a switcher.".into(),
    acceptance_criteria: vec!["The switcher works.".into()],
    pipeline: Some(ResourceId::generate()),
    limits: Some(
      TaskLimits::new(TokenBudget::new(1_400_000).expect("budget"), Duration::from_secs(1800))
        .expect("positive limits"),
    ),
  };
  let mut db = ProjectDb::open(&desktop.repo).expect("open project");
  let mut task = save_draft(&mut db, batch, details.clone(), Timestamp::now()).expect("save draft");
  task
    .queue(orchester_core::resource::VersionNumber::FIRST)
    .expect("complete task queues");
  db.save_task(&task).expect("persist queued task");
  assert_eq!(task.status(), TaskStatus::Queued);
  let result = save_draft_in(&desktop.repo, SaveTarget::New(batch), details);
  assert!(matches!(result, Err(TaskDraftSaveError::NotDraft(TaskStatus::Queued))));
}

#[test]
fn new_task_budget_starts_at_the_project_default_and_groups_digits_while_typing() {
  let mut desktop = new_task();
  assert_eq!(desktop.value(BUDGET), "1,400,000");
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

/// Right and bottom edges of an element.
pub(crate) fn ends(desktop: &mut Desktop, id: &str) -> (f32, f32) {
  let bounds = desktop.bounds(id);
  (bounds.x + bounds.width, bounds.y + bounds.height)
}

pub(crate) fn close(actual: f32, expected: f32) -> bool {
  (actual - expected).abs() < 0.5
}

#[test]
fn new_task_layout_ends_the_plan_with_its_actions_and_pins_the_footer() {
  let mut desktop = new_task();
  // The plan's details end where Create & run does; their scroll gutter is beside them
  // (`HVSv5` in `WOdDe`).
  let (selector_right, _) = ends(&mut desktop, "pipeline-selector");
  let (run_right, _) = ends(&mut desktop, "create-and-run");
  assert!(close(selector_right, run_right), "{selector_right} vs {run_right}");
  let (readback_right, _) = ends(&mut desktop, "task-budget-readback");
  assert!(close(readback_right, run_right), "{readback_right} vs {run_right}");
  // The instructions footer sits at the foot of the brief column, level with the plan's, with
  // Preview at its right (`MoDET`).
  let (footer_right, footer_bottom) = ends(&mut desktop, "new-task-preview-footer");
  let (_, plan_bottom) = ends(&mut desktop, "execution-plan");
  assert!(close(footer_bottom, plan_bottom), "{footer_bottom} vs {plan_bottom}");
  let (preview_right, _) = ends(&mut desktop, "task-preview");
  assert!(close(preview_right, footer_right), "{preview_right} vs {footer_right}");
}
