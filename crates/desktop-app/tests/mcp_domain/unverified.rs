//! Accepting a step that could not check everything passes it in a person's place, as approving does: `continue_run`'s
//! `accept` is refused as `automation_not_allowed` until a person turns on *Let automation answer approvals*, and so is
//! a client's press on *Accept … as it is* in the window, which the card says; the rest of the window takes the
//! client's input. `get_run` lists what the step could not check.

use orchester_core::store::{RunStore, TaskStore};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::{Served, kontur::Seeded};

/// The `decisions` scenario's run that waits unverified after Review, by number.
fn unverified_run(seeded: &Seeded) -> (String, String) {
  let task = seeded
    .tasks
    .iter()
    .find(|task| task.state == "waits unverified after Review")
    .expect("the seeded task");
  let db = ProjectDb::open(&seeded.repo).expect("the database opens");
  let id = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|candidate| candidate.key().to_string() == task.key)
    .expect("the task")
    .id();
  let run = db
    .runs_of_task(id)
    .expect("runs")
    .pop()
    .expect("its run")
    .expect("readable");
  (task.key.clone(), run.number().get().to_string())
}

fn accept(run: &str) -> Value {
  json!({ "project": "Kontur", "run": run, "choice": { "kind": "accept" } })
}

#[test]
fn domain_accepting_an_unverified_step_is_a_persons_unless_automation_may_answer() {
  let (mut served, seeded) = Served::seeded("decisions");
  let (_, run) = unverified_run(&seeded);
  let shown = served.ok("get_run", &json!({ "project": "Kontur", "run": run }));
  assert_eq!(shown["status"]["reason"], json!({ "unverified": "review" }), "{shown}");
  assert_eq!(shown["unverified"][0]["check"], "Tasks · a filter that matches nothing");
  let refused = served.refused("continue_run", &accept(&run));
  assert_eq!(refused["code"], "automation_not_allowed", "{refused}");
  served
    .services()
    .workspace
    .set_automation_answers_approvals(true)
    .expect("a person lets automation answer");
  // Past the person's consent the decision goes to the runner, which this app has none of.
  let sent = served.refused("continue_run", &accept(&run));
  assert_ne!(sent["code"], "automation_not_allowed", "{sent}");
}

#[test]
fn domain_window_input_cannot_accept_an_unverified_step() {
  let (folder, seeded) = Served::seed("decisions", None);
  let (key, run) = unverified_run(&seeded);
  // The real runner, so that the decision's buttons take presses (without one they are unavailable).
  let (runner, _) = super::live::programs();
  let mut served = Served::start_with(folder, &seeded.app_database, runner, true);
  served.ok(
    "navigate",
    &json!({ "route": "task", "project": seeded.project.to_string(), "key": key }),
  );
  served.until("Accept with the runner online", |served| {
    served.shows("task-accept-step") && !served.shows("task-retry-offline")
  });
  // Decision handlers refuse; the rest of the window still takes the client's input.
  served
    .click_by_client("task-retry-decision-title")
    .expect("window input is not refused while Accept shows");
  served
    .click_by_client("task-accept-step")
    .expect("the press reaches the button, whose handler refuses it");
  served.until("the refusal", |served| served.shows("task-decision-person-only"));
  let shown = served.ok("get_run", &json!({ "project": "Kontur", "run": run }));
  assert_eq!(
    shown["status"]["reason"],
    json!({ "unverified": "review" }),
    "nothing was accepted: {shown}"
  );
}
