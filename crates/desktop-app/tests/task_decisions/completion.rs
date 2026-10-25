//! A run that passed every step and committed nothing (ORC-93): when its task delivers a report, its Summary shows the
//! Ready card with *Complete task* and the run's report, and completing it says who completed the task and when (a
//! person, or automation); work done outside the repository lists the MCP tool calls it made; and where changes were
//! expected the run is flagged, with *Complete anyway*, *Return to a step* and *Run again*, and no Merge.

use orchester_core::{
  resource::Deliverable,
  run::{CompletedBy, Run, RunCompletion},
  store::TaskStore,
  task::organization::{ChangeActor, WorkflowStatus},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{
  Page, checkout_page, key_of,
  merge::{git, latest_run},
};

/// The seeded task that is ready for review and merges cleanly, its run's branch moved back to where the run started:
/// it committed nothing. The task delivers `delivers` (`None`: as its pipeline, changes).
fn committed_nothing(delivers: Option<Deliverable>) -> (Page, Run) {
  let page = Page::seeded("decisions");
  let state = "ready for review, merges cleanly";
  let run = latest_run(&page, state);
  let branch = format!("refs/heads/{}", run.branch());
  git(&page, &["update-ref", &branch, run.base().as_str()]);
  let key = key_of(&page, state);
  let mut db = ProjectDb::open(&page.repo).expect("the project database");
  let task = db
    .tasks()
    .expect("the tasks")
    .into_iter()
    .find(|task| task.key().to_string() == key)
    .expect("the seeded task");
  db.update_task_organization(task.id(), ChangeActor::Person, Timestamp::now(), |value| {
    value.with_delivers(delivers)
  })
  .expect("what the task delivers is kept");
  (page, run)
}

#[test]
fn decision_a_run_that_committed_nothing_where_changes_were_expected_is_flagged() {
  let _checkout = checkout_page();
  let (mut page, _) = committed_nothing(None);
  page.open("ready for review, merges cleanly", "task-notice");
  assert_eq!(
    page.text("task-notice-title"),
    "This run committed nothing, but its pipeline expects changes"
  );
  for action in [
    "task-complete-anyway",
    "task-complete-return",
    "task-complete-run-again",
  ] {
    assert!(page.has(action), "{action}");
  }
  assert!(!page.has("task-merge"), "there is no work to merge");
  assert!(!page.has("task-complete"), "only Complete anyway completes it");
}

#[test]
fn decision_a_report_run_is_completed_from_its_ready_card() {
  let _checkout = checkout_page();
  let (mut page, run) = committed_nothing(Some(Deliverable::Report));
  page.open("ready for review, merges cleanly", "task-complete");
  assert_eq!(page.text("task-notice-title"), "Ready to complete");
  assert!(page.text("task-notice").contains("Its result is its report"));
  assert!(page.has("task-report"), "the run's report is shown");
  assert!(!page.has("task-merge"), "Complete task in place of Merge");
  assert!(
    !page.has("task-outside"),
    "a report run made no changes outside the repository"
  );
  page.click("task-complete");
  page.until_says("task-notice-title", "Task completed");
  assert!(page.text("task-notice").contains("You completed the task"));
  assert!(!page.has("task-complete"));
  let db = ProjectDb::open(&page.repo).expect("the project database");
  assert!(db.run_completion(run.id()).expect("read").is_some());
  let task = db.task(run.task()).expect("read").expect("the task");
  assert_eq!(task.organization().status(), WorkflowStatus::Done);
}

#[test]
fn decision_a_run_automation_completed_says_so() {
  let _checkout = checkout_page();
  let (mut page, run) = committed_nothing(Some(Deliverable::Report));
  let completion = RunCompletion {
    run: run.id(),
    by: CompletedBy::Automation,
    completed_at: Timestamp::now(),
  };
  ProjectDb::open(&page.repo)
    .expect("the project database")
    .save_run_completion(&completion, None)
    .expect("the run is completed");
  page.open("ready for review, merges cleanly", "task-notice");
  assert_eq!(page.text("task-notice-title"), "Task completed");
  assert!(page.text("task-notice").contains("Automation completed the task"));
}

#[test]
fn decision_work_outside_the_repository_lists_its_tool_calls() {
  let _checkout = checkout_page();
  let (mut page, _) = committed_nothing(Some(Deliverable::Outside));
  page.open("ready for review, merges cleanly", "task-complete");
  assert!(
    page
      .text("task-notice")
      .contains("Its work was done outside the repository")
  );
  assert_eq!(page.text("task-outside-head"), "Changes outside the repository 0");
  assert!(
    page.has("task-outside-none"),
    "the seeded run was let make no MCP tool call"
  );
}
