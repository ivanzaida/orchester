//! `get_run` gives each step the todo list its latest execution's agent keeps (ORC-314): the latest update, as `text`
//! and `state` per item; `null` for a step whose agent keeps none.

use orchester_core::{
  connection::stream::{AgentEvent, TodoItem, TodoState},
  run::InvocationKind,
  store::{ActivityStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::{Served, kontur::Seeded};

/// The run of `title` with its agent at work writing two lists, the later one in place of the first. Answers the
/// run's id and the key of the step at work.
fn listed(seeded: &Seeded, title: &str) -> (String, String) {
  let mut db = ProjectDb::open(&seeded.repo).expect("the database opens");
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.details().title.as_str() == title)
    .expect("the seeded task");
  let run = db
    .runs_of_task(task.id())
    .expect("runs")
    .pop()
    .expect("its run")
    .expect("readable");
  let invocation = db
    .invocations(run.id())
    .expect("invocations")
    .into_iter()
    .find(|invocation| invocation.ended_at.is_none() && matches!(invocation.kind, InvocationKind::Agent { .. }))
    .expect("the agent at work");
  let item = |text: &str, state| TodoItem {
    text: text.to_owned(),
    state,
  };
  let first = vec![item("Read the sender", TodoState::InProgress)];
  let latest = vec![
    item("Read the sender", TodoState::Done),
    item("Add the retry", TodoState::InProgress),
    item("Run the tests", TodoState::Pending),
  ];
  let now = Timestamp::now();
  db.append_activity(
    &invocation,
    &[
      (now, AgentEvent::TodoList { items: first }),
      (now, AgentEvent::TodoList { items: latest }),
    ],
  )
  .expect("the lists are recorded");
  let step = run
    .execution(invocation.execution)
    .expect("its execution")
    .step()
    .to_string();
  (run.id().to_string(), step)
}

#[test]
fn domain_get_run_gives_each_step_its_latest_todo_list() {
  let (mut served, seeded) = Served::seeded("usage");
  let (run, at_work) = listed(&seeded, "Add retry to webhook sender");
  let shown = served.ok("get_run", &json!({ "project": "Kontur", "run": &run }));
  let steps = shown["steps"].as_array().expect("steps");
  let step = |key: &str| steps.iter().find(|step| step["key"] == key).expect("the step");
  assert_eq!(
    step(&at_work)["todo"],
    json!([
      {"text": "Read the sender", "state": "done"},
      {"text": "Add the retry", "state": "in_progress"},
      {"text": "Run the tests", "state": "pending"},
    ]),
    "{shown}"
  );
  let others: Vec<&Value> = steps.iter().filter(|step| step["key"] != at_work.as_str()).collect();
  assert!(
    others.iter().all(|step| step["todo"].is_null()),
    "a step whose agent keeps no list has none: {shown}"
  );
}
