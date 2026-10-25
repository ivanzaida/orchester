//! `complete_run` completes a run that passed every step and committed nothing as the task's Summary does (ORC-93),
//! recorded as automation's: a run whose result is its report at once; one whose pipeline expects changes only
//! `anyway`, and only while a person lets automation answer approvals, as an MCP client's press on *Complete anyway*
//! in the window, which is recorded as automation's too.

use orchester_core::{
  id::RunId,
  run::CompletedBy,
  store::{RunStore, TaskStore},
  task::organization::WorkflowStatus,
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::{Served, kontur::Seeded};

const REPORT: &str = "report run ready to complete";
const FLAGGED: &str = "committed nothing, changes expected";

/// The key and the run of the `deliverables` scenario's task left in `state`.
fn seeded_run(seeded: &Seeded, state: &str) -> (String, RunId) {
  let key = seeded
    .tasks
    .iter()
    .find(|task| task.state == state)
    .expect("the seeded task")
    .key
    .clone();
  let db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.key().to_string() == key)
    .expect("the task");
  let run = db
    .runs_of_task(task.id())
    .expect("runs")
    .pop()
    .expect("its run")
    .expect("readable");
  (key, run.id())
}

fn complete(run: RunId, anyway: bool) -> Value {
  json!({ "project": "Kontur", "run": run.to_string(), "anyway": anyway })
}

/// Who completed `run`, once it was.
fn completed_by(seeded: &Seeded, run: RunId) -> Option<CompletedBy> {
  ProjectDb::open(&seeded.repo)
    .expect("the project's data")
    .run_completion(run)
    .expect("the completion reads")
    .map(|completion| completion.by)
}

#[test]
fn domain_complete_run_completes_a_report_run_as_automation() {
  let (mut served, seeded) = Served::seeded("deliverables");
  let (_, run) = seeded_run(&seeded, REPORT);
  let answer = served.ok("complete_run", &complete(run, false));
  assert_eq!(answer["completion"]["by"], "automation", "{answer}");
  assert_eq!(completed_by(&seeded, run), Some(CompletedBy::Automation));
  let db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let ran = db.run(run).expect("the run reads").expect("the run");
  let task = db.task(ran.task()).expect("the task reads").expect("the task");
  assert_eq!(task.organization().status(), WorkflowStatus::Done);
  let again = served.refused("complete_run", &complete(run, false));
  assert_eq!(again["code"], "already_completed", "{again}");
}

#[test]
fn domain_complete_run_anyway_needs_the_persons_consent() {
  let (mut served, seeded) = Served::seeded("deliverables");
  let (_, run) = seeded_run(&seeded, FLAGGED);
  let plain = served.refused("complete_run", &complete(run, false));
  assert_eq!(plain["code"], "expects_changes", "{plain}");
  let anyway = served.refused("complete_run", &complete(run, true));
  assert_eq!(anyway["code"], "automation_not_allowed", "{anyway}");
  assert_eq!(completed_by(&seeded, run), None, "nothing was completed");
  served
    .services()
    .workspace
    .set_automation_answers_approvals(true)
    .expect("a person lets automation answer");
  served.ok("complete_run", &complete(run, true));
  assert_eq!(completed_by(&seeded, run), Some(CompletedBy::Automation));
}

#[test]
fn domain_window_input_completes_anyway_only_as_automation() {
  let (folder, seeded) = Served::seed("deliverables", None);
  let (key, run) = seeded_run(&seeded, FLAGGED);
  let runner = folder.path().join("no-runner").join("orchester-runner");
  let mut served = Served::start_with(folder, &seeded.app_database, runner, true);
  served.ok(
    "navigate",
    &json!({ "route": "task", "project": seeded.project.to_string(), "key": key, "tab": "summary" }),
  );
  served.until("Complete anyway", |served| served.shows("task-complete-anyway"));
  served
    .click_by_client("task-complete-anyway")
    .expect("the press reaches the button, whose handler refuses it");
  served.until("the refusal", |served| served.shows("task-complete-person-only"));
  assert_eq!(completed_by(&seeded, run), None, "nothing was completed");
  served
    .services()
    .workspace
    .set_automation_answers_approvals(true)
    .expect("a person lets automation answer");
  served
    .click_by_client("task-complete-anyway")
    .expect("the press is taken");
  served.until("the completion", |_| completed_by(&seeded, run).is_some());
  assert_eq!(completed_by(&seeded, run), Some(CompletedBy::Automation));
}
