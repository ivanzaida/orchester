use std::time::Duration;

use lurq::app::Tree;
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
  github::{contains_text, shows},
  harness::{Desktop, expected},
  task_run::set_text,
};

const TITLE: &str = "task-title";
const BRIEF: &str = "task-brief";
const FIRST_CRITERION: &str = "criterion-0";
const SECOND_CRITERION: &str = "criterion-1";
const ADD_CRITERION: &str = "add-criterion";
const BUDGET: &str = "task-budget";
const TIME_LIMIT: &str = "task-time-limit";
/// Create: saves the task without running it.
const CREATE: &str = "save-draft";
/// More than the window has stops on this screen.
const MAX_STOPS: usize = 30;

pub(crate) fn new_task() -> Desktop {
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
  // pipeline selector without pipelines, Create & run and Create (no title yet) cannot be used
  // and are skipped. After the form Tab wraps to its first field.
  assert_eq!(
    desktop.stops(11, false),
    expected(&[
      Some(TITLE),
      Some(BRIEF),
      Some(FIRST_CRITERION),
      Some(ADD_CRITERION),
      Some("task-workflow-field"),
      Some("task-priority-field"),
      Some("task-tags-picker"),
      Some("task-blocked-field"),
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

/// A criterion longer than the form's column, like ORC-20's.
pub(crate) const LONG_CRITERION: &str = "A criterion longer than the column wraps onto further lines in New task and \
                                         Edit task, grows the row, and keeps its radio and remove control on the \
                                         first line, so its end stays visible while typing and after saving.";

#[test]
fn new_task_enter_adds_a_criterion_and_backspace_removes_an_empty_one() {
  let mut desktop = new_task();
  desktop.tab_to(FIRST_CRITERION, MAX_STOPS);
  desktop.type_text("a");
  desktop.enter();
  desktop.settle("the added criterion to take the focus", |tree| {
    tree
      .focused_element()
      .is_some_and(|element| element.id() == Some(SECOND_CRITERION))
  });
  assert_eq!(
    desktop.value(FIRST_CRITERION),
    "a",
    "Enter puts no line break in a criterion"
  );
  desktop.type_text("b");
  assert_eq!(desktop.value(SECOND_CRITERION), "b");
  // Backspace edits a criterion that has text; in an empty one it removes the row.
  desktop.key("Backspace", "Backspace");
  assert_eq!(desktop.value(SECOND_CRITERION), "");
  desktop.key("Backspace", "Backspace");
  desktop.settle("the empty criterion to go", |tree| {
    tree.get_element_by_id(SECOND_CRITERION).is_none()
      && tree
        .focused_element()
        .is_some_and(|element| element.id() == Some(FIRST_CRITERION))
  });
  assert_eq!(desktop.value(FIRST_CRITERION), "a");
  // The last criterion stays, empty.
  desktop.key("Backspace", "Backspace");
  desktop.key("Backspace", "Backspace");
  assert_eq!(desktop.value(FIRST_CRITERION), "");
  assert!(desktop.tree.get_element_by_id(FIRST_CRITERION).is_some());
}

#[test]
fn new_task_long_criterion_wraps_in_its_row_and_is_saved_as_typed() {
  let mut desktop = new_task();
  desktop.tab_to(TITLE, MAX_STOPS);
  desktop.type_text("Wrap criteria");
  desktop.tab_to(FIRST_CRITERION, MAX_STOPS);
  desktop.type_text("Short");
  let line = desktop.bounds(FIRST_CRITERION);
  desktop.enter();
  desktop.wait_for(SECOND_CRITERION);
  set_text(&mut desktop, SECOND_CRITERION, LONG_CRITERION);
  desktop.type_text(" More.");
  let typed = format!("{LONG_CRITERION} More.");
  assert_eq!(desktop.value(SECOND_CRITERION), typed);
  // The long criterion wraps within the column and grows its row; the short one keeps one line.
  let long = desktop.bounds(SECOND_CRITERION);
  assert!(long.height > 1.5 * line.height, "{} vs {}", long.height, line.height);
  assert!(close(desktop.bounds(FIRST_CRITERION).height, line.height));
  let (column_right, _) = ends(&mut desktop, "new-task-brief");
  let (remove_right, _) = ends(&mut desktop, "remove-criterion-1");
  assert!(long.x + long.width <= remove_right && remove_right <= column_right);
  // Its remove control stays beside its first line.
  let remove = desktop.bounds("remove-criterion-1");
  let middle = remove.y + remove.height / 2.0;
  assert!(close(middle, long.y + line.height / 2.0), "{middle} vs {}", long.y);
  desktop.press(CREATE);
  desktop.settle("the new task in the list", |tree| {
    tree.get_element_by_id(TITLE).is_none() && highlighted(tree, "KON-1")
  });
  let db = ProjectDb::open(&desktop.repo).expect("open project");
  let tasks = db.tasks().expect("read tasks");
  assert_eq!(tasks[0].details().acceptance_criteria, ["Short".to_owned(), typed]);
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

/// Whether Create can be pressed: Tab skips it while it cannot.
fn create_available(desktop: &mut Desktop) -> bool {
  desktop.stops(MAX_STOPS, false).contains(&Some(CREATE.to_owned()))
}

/// Whether the task list highlights the row of `key`, and only it, as the task Create just made.
pub(crate) fn highlighted(tree: &mut Tree, key: &str) -> bool {
  let mut rows = tree.get_elements_by_class_name("created").into_iter();
  rows.next().is_some_and(|row| contains_text(row, key)) && rows.next().is_none()
}

#[test]
fn new_task_enter_creates_only_a_valid_task() {
  let mut desktop = new_task();
  desktop.tab_to(TITLE, MAX_STOPS);
  desktop.type_text("Add switcher");
  desktop.tab_to(BUDGET, MAX_STOPS);
  desktop.type_text("x");
  assert_eq!(desktop.value(BUDGET), "1,400,000x");
  // The budget is not a number: Create is disabled and Enter creates nothing.
  desktop.enter();
  desktop.settle("the form to stay", |tree| tree.get_element_by_id(TITLE).is_some());
  desktop.key("Backspace", "Backspace");
  desktop.tab_to(TITLE, MAX_STOPS);
  desktop.enter();
  desktop.settle("the new task in the list", |tree| {
    tree.get_element_by_id(TITLE).is_none() && highlighted(tree, "KON-1")
  });
  let db = ProjectDb::open(&desktop.repo).expect("open project");
  assert_eq!(db.tasks().expect("read tasks").len(), 1);
}

#[test]
fn new_task_create_returns_to_the_list_with_the_task_highlighted() {
  let mut desktop = new_task();
  assert!(shows(&mut desktop, CREATE, "Create"));
  desktop.tab_to(TITLE, MAX_STOPS);
  desktop.type_text("Add switcher");
  desktop.tab_to(BRIEF, MAX_STOPS);
  desktop.type_text("Add a switcher to the project.");
  desktop.press(CREATE);
  desktop.settle("the new task in the list", |tree| {
    tree.get_element_by_id(TITLE).is_none() && highlighted(tree, "KON-1")
  });
  assert!(shows(&mut desktop, "task-row-KON-1", "Add switcher"));
  assert!(shows(&mut desktop, "task-row-KON-1", "Backlog"));
  let db = ProjectDb::open(&desktop.repo).expect("open project");
  let tasks = db.tasks().expect("read tasks");
  assert_eq!(tasks.len(), 1);
  assert_eq!(tasks[0].status(), TaskStatus::Draft, "Create does not run the task");
  assert_eq!(tasks[0].details().brief, "Add a switcher to the project.");
  // The highlight marks only the task just made: opening the list again shows none.
  desktop.press("nav-overview");
  desktop.wait_for("new-task");
  desktop.press("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  assert!(desktop.tree.get_elements_by_class_name("created").is_empty());
}

#[test]
fn new_task_create_is_unavailable_while_the_form_is_invalid() {
  let mut desktop = new_task();
  // No title yet.
  assert!(!create_available(&mut desktop));
  desktop.tab_to(TITLE, MAX_STOPS);
  desktop.type_text("Add switcher");
  assert!(create_available(&mut desktop));
  desktop.tab_to(TIME_LIMIT, MAX_STOPS);
  desktop.type_text("x");
  assert!(!create_available(&mut desktop));
  assert!(shows(
    &mut desktop,
    "execution-plan",
    "Correct the run budget and time limit to save the task."
  ));
  desktop.click(CREATE);
  desktop.settle("the form to stay", |tree| tree.get_element_by_id(TITLE).is_some());
  let db = ProjectDb::open(&desktop.repo).expect("open project");
  assert!(db.tasks().expect("read tasks").is_empty());
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
  let mut task = save_draft(&mut db, batch, details.clone(), Timestamp::now())
    .expect("save draft")
    .saved()
    .expect("a new draft");
  task
    .queue(orchester_core::resource::VersionNumber::FIRST)
    .expect("complete task queues");
  db.save_task(&task)
    .expect("persist queued task")
    .saved()
    .expect("saved as read");
  assert_eq!(task.status(), TaskStatus::Queued);
  let result = save_draft_in(&desktop.repo, SaveTarget::New(batch), &details);
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
  // Preview at its right, where the fields end (`MoDET`).
  let (_, footer_bottom) = ends(&mut desktop, "new-task-preview-footer");
  let (_, plan_bottom) = ends(&mut desktop, "execution-plan");
  assert!(close(footer_bottom, plan_bottom), "{footer_bottom} vs {plan_bottom}");
  let (preview_right, _) = ends(&mut desktop, "task-preview");
  let (title_right, _) = ends(&mut desktop, TITLE);
  assert!(close(preview_right, title_right), "{preview_right} vs {title_right}");
}
