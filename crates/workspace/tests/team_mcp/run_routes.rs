//! A run is a team task's by the run itself: its team record, its task's team identity or the team inputs it was
//! created with. One of the last two without the record (a legacy run from before team runs were recorded, or a start
//! that stopped before its record was written) takes neither route: no tool merges, continues or resumes it, and no
//! local merge is tried for it. A local task's run in a team-linked project stays a local run.

use orchester_core::{
  id::{RunId, TaskId},
  store::{RunStore, TaskStore},
};
use orchester_exec::programs::Programs;
use orchester_workspace::tasks::{
  integration::{IntegrationProblem, merge_in},
  recorded::{RunRoute, route_of_run},
  worktree_removal::WorktreeRemover,
};
use serde_json::{Value, json};

use crate::{
  Checkout,
  team_runs::started_without_record,
  tools::{canceled_run, server},
  unrecorded::legacy,
};

fn route(checkout: &Checkout, run: RunId) -> RunRoute {
  let db = checkout.db();
  let run = db.run(run).expect("read").expect("the run");
  route_of_run(&db, &run).expect("route")
}

/// `merge_run`, `continue_run` and `resume_run` of `run`, each with the arguments it takes.
fn takes(run: RunId) -> [(&'static str, Value); 3] {
  let run = run.to_string();
  [
    ("merge_run", json!({"project": "Kontur", "run": run})),
    (
      "continue_run",
      json!({"project": "Kontur", "run": run, "choice": {"kind": "resume"}}),
    ),
    ("resume_run", json!({"project": "Kontur", "run": run})),
  ]
}

/// The local merge of the task page is refused as a team run's and records no attempt.
fn no_local_merge(checkout: &Checkout, run: RunId) {
  let programs = Programs::processes();
  let worktrees = WorktreeRemover::here_for_test(checkout.root(), programs.clone());
  let refused = merge_in(&worktrees, &programs, (run, None)).map(|merged| merged.merge);
  assert!(
    matches!(&refused, Err(IntegrationProblem::Store(message)) if message.key == "localIntegration.teamRun"),
    "{refused:?}"
  );
  let db = checkout.db();
  assert!(db.merge_attempts(run).expect("attempts").is_empty());
  assert!(db.open_merge_attempts().expect("open").is_empty());
}

#[test]
fn team_mcp_a_legacy_team_run_with_no_team_record_is_refused_by_every_route() {
  let (checkout, _server, _task, run) = legacy();
  assert!(checkout.db().team_run(run).expect("read").is_none(), "no team record");
  assert_eq!(route(&checkout, run), RunRoute::TeamUnrecorded);

  for (tool, args) in takes(run) {
    assert_eq!(checkout.refused(tool, &args), "team_run_unrecorded", "{tool}");
  }
  // Said by the run's task being unrecorded, as before: nothing is stored or sent.
  let message = json!({"project": "Kontur", "run": run.to_string(), "text": "Go on."});
  assert_eq!(checkout.refused("send_message", &message), "team_task_unrecorded");
  no_local_merge(&checkout, run);
}

#[test]
fn team_mcp_a_run_with_team_inputs_and_no_team_record_is_a_team_run_that_takes_no_route() {
  // The runner stopped between creating the run and recording it: the inputs stay, the record is not there.
  let team = started_without_record();
  assert!(team.checkout.db().team_run(team.run).expect("read").is_none());
  assert_eq!(route(&team.checkout, team.run), RunRoute::TeamUnrecorded);

  for (tool, args) in takes(team.run) {
    assert_eq!(team.checkout.refused(tool, &args), "team_run_unrecorded", "{tool}");
  }
  let message = json!({"project": "Kontur", "run": team.run.to_string(), "text": "Go on."});
  assert_eq!(team.checkout.refused("send_message", &message), "team_run_unrecorded");
  no_local_merge(&team.checkout, team.run);
}

#[test]
fn team_mcp_a_run_whose_only_mark_is_its_team_inputs_is_a_team_run() {
  let team = started_without_record();
  let connection = rusqlite::Connection::open(team.checkout.root().join(".orchester/orchester.db")).expect("open");
  let task: TaskId = team.checkout.db().run(team.run).expect("read").expect("run").task();
  connection
    .execute_batch(&format!(
      "PRAGMA foreign_keys = OFF; UPDATE task_identities SET owner = 'local', server_id = NULL, team_project_id = NULL, \
       team_task_id = NULL, recorded_revision = NULL WHERE id = '{task}';"
    ))
    .expect("the task is no team task's");
  drop(connection);

  assert!(!orchester_workspace::tasks::recorded::is_team(&team.checkout.db(), task).expect("read"));
  assert_eq!(route(&team.checkout, team.run), RunRoute::TeamUnrecorded);
}

#[test]
fn team_mcp_a_local_runs_merge_in_a_team_project_is_decided_as_a_local_merge() {
  let (server, _seen) = server();
  let checkout = Checkout::prepared(Some(&server), &["Say hello"], canceled_run);
  let task = checkout.db().tasks().expect("tasks")[0].id();
  let run = checkout.db().runs_of_task(task).expect("runs")[0]
    .as_ref()
    .expect("readable")
    .id();
  assert_eq!(route(&checkout, run), RunRoute::Local);

  // The local rules answer (a canceled run has nothing to merge), not a team refusal.
  let code = checkout.refused("merge_run", &json!({"project": "Kontur", "run": run.to_string()}));
  assert!(!code.starts_with("team_"), "{code}");
}
