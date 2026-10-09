//! A saved task's brief and acceptance criteria on its Summary, and a draft reopened in the New
//! task form (design doc, "ORC-12 · Draft Summary and editing"): Edit fills the form, *Save changes*
//! saves over the same task, *Create & run* starts it, and leaving with changes that are not saved
//! asks first. Editing after a run starts changes future runs, while the existing run keeps its task.

use std::{path::Path, time::Duration};

use lurq::app::events::MouseButton;
use orchester_core::{
  id::IdempotencyKey,
  name::Name,
  resource::ResourceKind,
  store::{ResourceStore, TaskStore},
  task::{TaskDetails, TaskLimits, TaskStatus},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_store::ProjectDb;

use crate::{
  github::shows,
  harness::Desktop,
  new_task::{LONG_CRITERION, close, ends},
  tabs::{BACK, click_with, command, has, side_button},
  task_run::{init_repository, latest_run, programs, seed_claude, set_text},
};

const ROW: &str = "task-row-KON-1";

fn draft(title: &str) -> TaskDetails {
  TaskDetails {
    title: Name::new(title).expect("title"),
    brief: "Let users sign in with GitHub.".into(),
    acceptance_criteria: vec!["A new user can sign in.".into(), "Email sign-in still works.".into()],
    pipeline: None,
    limits: TaskLimits::NONE,
  }
}

fn save(repo: &Path, details: TaskDetails) {
  ProjectDb::open(repo)
    .expect("open the project")
    .create_tasks(IdempotencyKey::generate(), vec![details], Timestamp::now())
    .expect("the draft is saved");
}

fn stored(repo: &Path) -> Vec<orchester_core::task::Task> {
  ProjectDb::open(repo).expect("open the project").tasks().expect("tasks")
}

/// Kontur with the draft KON-1 open on its page.
pub(crate) fn draft_page() -> Desktop {
  let mut desktop = Desktop::with_seeded_project(|repo| save(repo, draft("Add GitHub authentication")));
  open_task(&mut desktop);
  desktop
}

fn open_task(desktop: &mut Desktop) {
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for(ROW);
  desktop.press(ROW);
  desktop.wait_for("task-this-run");
}

pub(crate) fn edit(desktop: &mut Desktop) {
  desktop.wait_for("task-edit");
  desktop.click("task-edit");
  desktop.wait_for("task-title");
}

fn state_reads(desktop: &mut Desktop, text: &str) {
  desktop.settle(text, |tree| {
    tree
      .get_element_by_id("new-task-state")
      .and_then(|state| state.text_content())
      .is_some_and(|state| state.contains(text))
  });
}

/// Whether Save changes can be pressed: Tab skips it while it cannot.
fn save_available(desktop: &mut Desktop) -> bool {
  desktop.stops(20, false).contains(&Some("save-draft".to_owned()))
}

#[test]
fn edit_task_draft_summary_shows_the_brief_criteria_and_this_task() {
  let mut desktop = draft_page();
  desktop.wait_for("task-brief");
  assert!(shows(&mut desktop, "task-notice", "This task is a draft"));
  assert!(shows(&mut desktop, "task-brief", "Let users sign in with GitHub."));
  assert!(shows(&mut desktop, "task-criteria", "1"));
  assert!(shows(&mut desktop, "task-criteria", "Email sign-in still works."));
  assert!(shows(&mut desktop, "task-this-run", "This task"));
  assert!(shows(&mut desktop, "task-this-run", "No pipeline chosen"));
  assert!(shows(&mut desktop, "task-this-run", "No limits set"));
  assert!(shows(&mut desktop, "task-edit", "Edit"));
}

#[test]
fn edit_task_long_criterion_wraps_on_the_summary_and_in_the_form() {
  let mut desktop = Desktop::with_seeded_project(|repo| {
    save(
      repo,
      TaskDetails {
        acceptance_criteria: vec!["A new user can sign in.".into(), LONG_CRITERION.into()],
        ..draft("Add GitHub authentication")
      },
    );
  });
  open_task(&mut desktop);
  desktop.wait_for("task-criterion-2");
  // On the Summary the long criterion wraps beside its number, within the criteria's column.
  let short = desktop.bounds("task-criterion-1");
  let long = desktop.bounds("task-criterion-2");
  assert!(long.height > 1.5 * short.height, "{} vs {}", long.height, short.height);
  let (column_right, _) = ends(&mut desktop, "task-criteria");
  assert!(long.x + long.width <= column_right + 0.5);
  assert!(shows(
    &mut desktop,
    "task-criteria",
    "so its end stays visible while typing and after saving."
  ));

  edit(&mut desktop);
  let short = desktop.bounds("criterion-0");
  let long = desktop.bounds("criterion-1");
  assert!(long.height > 1.5 * short.height, "{} vs {}", long.height, short.height);
  assert_eq!(desktop.value("criterion-1"), LONG_CRITERION);
  set_text(&mut desktop, "task-title", "Add GitHub sign-in");
  desktop.press("save-draft");
  state_reads(&mut desktop, "Draft saved locally");
  assert_eq!(
    stored(&desktop.repo)[0].details().acceptance_criteria,
    ["A new user can sign in.".to_owned(), LONG_CRITERION.to_owned()],
    "saving keeps the criterion as it was, without line breaks"
  );
}

#[test]
fn edit_task_opens_the_filled_form_and_saves_changes_in_place() {
  let mut desktop = draft_page();
  edit(&mut desktop);
  assert!(shows(&mut desktop, "task-form", "Edit task"));
  assert!(shows(&mut desktop, "task-form", "Kontur / Tasks / KON-1 / Edit"));
  assert!(shows(
    &mut desktop,
    "task-form",
    "Changes apply to draft KON-1. Nothing runs until you choose Create & run."
  ));
  assert_eq!(desktop.value("task-title"), "Add GitHub authentication");
  assert_eq!(desktop.value("task-brief"), "Let users sign in with GitHub.");
  assert_eq!(desktop.value("criterion-0"), "A new user can sign in.");
  assert_eq!(desktop.value("criterion-1"), "Email sign-in still works.");
  assert_eq!(
    desktop.value("task-budget"),
    "",
    "a draft without limits keeps them empty"
  );
  state_reads(&mut desktop, "Draft saved locally");
  assert!(shows(&mut desktop, "save-draft", "Save changes"));
  assert!(!save_available(&mut desktop), "nothing differs from the saved draft");

  set_text(&mut desktop, "task-title", "Add GitHub sign-in");
  state_reads(&mut desktop, "Unsaved changes");
  assert!(save_available(&mut desktop));
  desktop.press("save-draft");
  state_reads(&mut desktop, "Draft saved locally");
  assert!(!save_available(&mut desktop));
  // Unlike Create, Save changes keeps the form open.
  assert!(shows(&mut desktop, "save-draft", "Save changes"));
  assert!(shows(&mut desktop, "task-form", "Edit task"));
  assert_eq!(desktop.value("task-title"), "Add GitHub sign-in");

  let tasks = stored(&desktop.repo);
  assert_eq!(tasks.len(), 1, "the draft was saved over, not added");
  assert_eq!(tasks[0].key().to_string(), "KON-1");
  assert_eq!(tasks[0].details().title.as_str(), "Add GitHub sign-in");
  assert_eq!(tasks[0].status(), TaskStatus::Draft);
}

#[test]
fn edit_task_leaving_with_unsaved_changes_asks_first() {
  let mut desktop = draft_page();
  edit(&mut desktop);
  // Unchanged, the form is left without asking.
  desktop.press("new-task-back");
  desktop.wait_for("task-page");
  assert!(!has(&mut desktop, "discard-changes-dialog"));

  edit(&mut desktop);
  set_text(&mut desktop, "task-title", "Add GitHub sign-in");
  desktop.press("new-task-back");
  desktop.wait_for("discard-changes-dialog");
  assert!(shows(&mut desktop, "discard-changes-dialog", "Discard changes?"));
  assert!(shows(
    &mut desktop,
    "discard-changes-dialog",
    "Your edits to KON-1 are not saved yet."
  ));
  assert!(shows(&mut desktop, "discard-changes-keep", "Keep editing"));
  desktop.key("Escape", "Escape");
  assert!(!has(&mut desktop, "discard-changes-dialog"), "Escape keeps editing");
  assert_eq!(desktop.value("task-title"), "Add GitHub sign-in");

  // Another page of the tab and Back ask too.
  desktop.click("nav-overview");
  desktop.wait_for("discard-changes-dialog");
  desktop.press("discard-changes-keep");
  assert!(has(&mut desktop, "task-title"));
  side_button(&mut desktop, BACK);
  desktop.wait_for("discard-changes-dialog");
  assert!(shows(&mut desktop, "discard-changes-confirm", "Discard changes"));
  desktop.press("discard-changes-confirm");
  desktop.wait_for("task-page");
  assert!(!has(&mut desktop, "task-title"));
  assert_eq!(
    stored(&desktop.repo)[0].details().title.as_str(),
    "Add GitHub authentication",
    "the saved draft stays as it was"
  );
}

/// The published Delivery pipeline and the draft KON-1 on it, with a budget its run stops at.
fn seed_runnable_draft(repo: &Path, fake: &Path) {
  seed_claude(repo, fake);
  let db = ProjectDb::open(repo).expect("open the project");
  let pipeline = db.resources(ResourceKind::Pipeline).expect("pipelines")[0].id();
  drop(db);
  let limits = TaskLimits::new(
    Some(TokenBudget::new(5_000).expect("budget")),
    Some(Duration::from_secs(1_800)),
  )
  .expect("limits");
  save(
    repo,
    TaskDetails {
      title: Name::new("Say hello").expect("title"),
      brief: "Work in turns.".into(),
      acceptance_criteria: vec!["README.md ends with hello".into()],
      pipeline: Some(pipeline),
      limits,
    },
  );
}

#[test]
fn edit_task_create_and_run_keeps_the_run_snapshot_when_edited_again() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed_runnable_draft(repo, &fake));
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for(ROW);
  // Tab 2 shows the draft's page too; tab 1 opens Edit task on it.
  click_with(&mut desktop, ROW, MouseButton::Left, (true, false));
  desktop.settle("a background tab", |tree| {
    tree.get_element_by_id("window-tab-2").is_some()
  });
  desktop.press(ROW);
  desktop.wait_for("task-this-run");
  assert!(shows(&mut desktop, "task-this-run", "Delivery · v1"));
  assert!(shows(&mut desktop, "task-this-run", "5k tokens"));
  edit(&mut desktop);

  command(&mut desktop, "2", "Digit2", false);
  edit(&mut desktop);
  set_text(&mut desktop, "task-title", "Say hello twice");
  desktop.wait_until("the task page with its run", |desktop| {
    if !has(desktop, "task-page") && has(desktop, "create-and-run") {
      desktop.click("create-and-run");
    }
    has(desktop, "task-page")
  });
  desktop.wait_until("the run stopped at its limit", |desktop| has(desktop, "raise-limit"));
  desktop.wait_for("task-brief");
  assert!(shows(&mut desktop, "task-brief", "Work in turns."));
  assert!(shows(&mut desktop, "task-criteria", "README.md ends with hello"));
  assert!(shows(&mut desktop, "task-edit", "Edit"));
  let tasks = stored(&desktop.repo);
  assert_eq!(tasks.len(), 1, "Create & run started the edited draft");
  assert_eq!(tasks[0].key().to_string(), "KON-1");
  assert_eq!(tasks[0].details().title.as_str(), "Say hello twice");
  let run = latest_run(&desktop.repo).expect("the run was started");

  edit(&mut desktop);
  assert!(shows(
    &mut desktop,
    "task-form",
    "Changes to KON-1 apply to future runs. Existing runs keep the task details they started with."
  ));
  assert!(!has(&mut desktop, "create-and-run"));
  // Save changes is the plan's last line, its label at the column's foot (`lYOiy` in `dRN4m`).
  let (_, save_bottom) = ends(&mut desktop, "save-draft");
  let (_, plan_bottom) = ends(&mut desktop, "execution-plan");
  assert!(close(save_bottom, plan_bottom), "{save_bottom} vs {plan_bottom}");
  set_text(&mut desktop, "task-title", "Say hello three times");
  set_text(&mut desktop, "task-brief", "Work in future turns.");
  desktop.press("save-draft");
  state_reads(&mut desktop, "Task saved locally");
  assert_eq!(
    stored(&desktop.repo)[0].details().title.as_str(),
    "Say hello three times"
  );

  desktop.press("new-task-back");
  desktop.wait_for("task-page");
  desktop.wait_for("task-brief");
  assert!(shows(&mut desktop, "task-brief", "Work in turns."));
  let db = ProjectDb::open(&desktop.repo).expect("open the project");
  let original = db
    .task_for_run(tasks[0].id(), run.id())
    .expect("read run task")
    .expect("the run task exists");
  assert_eq!(original.details().title.as_str(), "Say hello twice");
  assert_eq!(original.details().brief, "Work in turns.");
}

#[test]
fn edit_task_create_and_run_the_runner_refuses_offers_retry_run() {
  // No runner program exists here, so the start request fails once the task is queued.
  let mut desktop = Desktop::with_seeded_project(|repo| seed_runnable_draft(repo, &repo.join("codex.exe")));
  open_task(&mut desktop);
  edit(&mut desktop);
  desktop.wait_until("Create & run", |desktop| {
    has(desktop, "create-and-run") && shows(desktop, "create-and-run", "Create & run")
  });
  desktop.click("create-and-run");
  state_reads(&mut desktop, "Queued · run not started");
  assert_eq!(stored(&desktop.repo)[0].status(), TaskStatus::Queued);
  // Once the project's tasks are read again with KON-1 queued, the form still offers the retry:
  // no run was started that the form could only edit for future runs.
  desktop.wait_until("the queued task counted", |desktop| {
    shows(desktop, "status-queued", "1")
  });
  assert!(has(&mut desktop, "create-and-run"));
  assert!(shows(&mut desktop, "create-and-run", "Retry run"));
  assert!(shows(&mut desktop, "execution-plan", "The runner is not connected."));
  assert!(!has(&mut desktop, "save-draft"));
  // The queued draft reads as the queued task of D5, as when it is reopened (`WOdDe`, `T6bVyU`).
  assert!(shows(&mut desktop, "task-form", "What would you like to build?"));
  assert!(shows(&mut desktop, "task-form", "Kontur / Tasks / KON-1 / Retry run"));
  assert!(!shows(&mut desktop, "task-form", "Edit task"));
  assert!(!shows(&mut desktop, "task-form", "KON-1 / Edit"));
  // Add criterion keeps its place, unavailable: Tab passes it by.
  assert!(has(&mut desktop, "add-criterion"));
  assert!(!desktop.stops(20, false).contains(&Some("add-criterion".to_owned())));
}
