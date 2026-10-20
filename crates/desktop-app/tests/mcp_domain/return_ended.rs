//! Returning the work of a run that ended from its task page, with the real runner (the project task ORC-44 · Return
//! a finished run to an earlier step): the person picks Implement on *Return the work to an earlier step* of a
//! succeeded and of a failed run, writes the note, and the runner reopens the run from that step with it, recorded as
//! the person's. An MCP client's press is refused while the person does not let automation answer, and nothing is
//! returned; with their leave it is recorded as automation's.

use orchester_core::{
  run::{Answerer, AttemptReason, RunEventKind},
  store::{RunStore, SettingsStore},
};
use orchester_store::ProjectDb;
use serde_json::json;

use super::{
  Served,
  resolve::{latest, wait_for},
};

const NOTE: &str = "Show the newest delivery first.";

/// Opens the Summary of the task left in `state` of `scenario` with the real runner, returns its latest run to
/// Implement with [`NOTE`] by `by`, and checks what the runner recorded.
fn returns_from_the_page(scenario: &str, state: &str, by: Answerer) {
  let (folder, seeded) = Served::seed(scenario, None);
  let (key, run) = latest(&seeded, state);
  // `task-states` spends the project's daily budget for its run stopped by it; the person lifts it, so the return goes
  // to work.
  let mut db = ProjectDb::open(&seeded.repo).expect("the database opens");
  let limits = db.project_limits().expect("limits").with_daily_budget_limit(None);
  db.save_project_limits(&limits).expect("no daily limit");
  drop(db);
  let runner = super::runner_programs::programs().runner.clone();
  let mut served = Served::start_with(folder, &seeded.app_database, runner, true);
  served.ok(
    "navigate",
    &json!({ "route": "task", "project": seeded.project.to_string(), "key": key, "tab": "summary" }),
  );
  served.until("the return, online", |served| {
    served.shows("task-return-ended-button") && !served.shows("task-return-ended-reason")
  });
  choose(&mut served, "task-return-ended-step", "03 Implement");
  served.set_by_client("task-return-ended-note", NOTE);
  // Returning a run decides for the person: a client's press waits for their leave. The button takes presses once the
  // run's worktree was read, so the client presses until one is taken.
  served.until("the refusal", |served| {
    if served.shows("task-return-ended-person-only") {
      return true;
    }
    served
      .click_by_client("task-return-ended-button")
      .expect("the press reaches the button");
    false
  });
  assert_eq!(
    wait_for(&mut served, &seeded, run.id(), |_| true).attempts().len(),
    run.attempts().len(),
    "nothing was returned"
  );
  match by {
    Answerer::Person => served.click_by_person("task-return-ended-button"),
    Answerer::Automation => {
      served
        .services()
        .workspace
        .set_automation_answers_approvals(true)
        .expect("a person lets automation answer");
      served
        .click_by_client("task-return-ended-button")
        .expect("the press reaches the button");
    }
  }
  served.until("the runner took the return", |served| {
    assert!(
      !served.shows("task-return-ended-refused"),
      "{}",
      super::tools_window::text(served, "task-return-ended-refused")
    );
    !served.shows("task-return-ended")
      || ProjectDb::open(&seeded.repo)
        .expect("the database opens")
        .run(run.id())
        .expect("read")
        .is_some_and(|stored| stored.attempts().len() > run.attempts().len())
  });
  let returned = wait_for(&mut served, &seeded, run.id(), |stored| {
    stored.attempts().len() > run.attempts().len()
  });
  assert_eq!(
    returned.attempts()[run.attempts().len()].reason(),
    &AttemptReason::ReturnedAfterEnd {
      return_to: orchester_core::resource::StepKey::new("implement").expect("a step key"),
      note: orchester_core::run::ReviewNote::new(NOTE).expect("a note"),
      conflicts: None,
    }
  );
  let events = ProjectDb::open(&seeded.repo)
    .expect("the database opens")
    .events(run.id())
    .expect("events");
  assert!(
    events
      .iter()
      .any(|event| matches!(event.kind(), RunEventKind::ReturnDecided { by: decided, .. } if *decided == by)),
    "recorded as {by:?}'s"
  );
}

/// Picks the option `label` of the select `id`.
fn choose(served: &mut Served, id: &str, label: &str) {
  let mut select = served
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.as_select())
    .unwrap_or_else(|| panic!("{id} is a select"));
  let index = select
    .labels()
    .iter()
    .position(|option| &**option == label)
    .unwrap_or_else(|| panic!("an option {label}"));
  select.commit(index);
  served.pass();
}

#[test]
fn domain_return_ended_the_person_returns_a_succeeded_run_from_its_summary() {
  returns_from_the_page("decisions", "ready for review, merges cleanly", Answerer::Person);
}

#[test]
fn domain_return_ended_the_person_returns_a_failed_run_from_its_summary() {
  returns_from_the_page("task-states", "failed", Answerer::Person);
}

#[test]
fn domain_return_ended_automation_returns_a_run_only_with_the_persons_leave() {
  returns_from_the_page("task-states", "failed", Answerer::Automation);
}
