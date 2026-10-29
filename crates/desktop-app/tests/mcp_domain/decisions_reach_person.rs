//! Decisions always reach the person, whatever the approval mode (ORC-348, `docs/design/approval-modes.md`,
//! "Decisions"): with auto mode on for the project and every run, and *approve everything in each run* on, an
//! MCP client's answer to a question, a plan, a run that waits for a person, a limit or a merge's conflicts is
//! refused until the person lets automation answer approvals, and the Orchestrator chat only proposes them until
//! its own switch and that one are on. The Summary's limit card is in `person_decisions::limits`.

use std::path::Path;

use orchester_core::{
  run::{Answerer, ApproveEverything, review::AutoModeSettings},
  store::{RunStore, TaskStore},
};
use orchester_store::ProjectDb;
use orchester_workspace::{app_mcp::ToolError, workspace::OrchestratorConsent};
use serde_json::{Value, json};

use super::{
  Served,
  orchestrator_chat::{allow, chat, chat_call, task_run},
};

/// Today's nearest to Autopilot: new runs and every run of the project at `repo` use auto mode, and auto mode
/// approves everything in each run, as the person turned it on.
pub(super) fn autopilot(repo: &Path) {
  let mut db = ProjectDb::open(repo).expect("the project's data");
  db.update_auto_mode_settings(|current| AutoModeSettings {
    new_runs: true,
    reviewer: current.reviewer,
  })
  .expect("auto mode for new runs");
  db.save_approve_everything(ApproveEverything::on(Answerer::Person))
    .expect("approve everything in each run");
  for task in db.tasks().expect("tasks") {
    for run in db.runs_of_task(task.id()).expect("runs").into_iter().flatten() {
      db.set_run_auto_mode(run.id(), true).expect("the run's auto mode");
    }
  }
}

/// `continue_run` on a run the tools find nowhere, with `choice`: the consent is checked before the run is read.
fn continuing(choice: &Value) -> (&'static str, Value) {
  (
    "continue_run",
    json!({ "project": "Kontur", "run": "999", "choice": choice }),
  )
}

/// Every decision kind an MCP client may take on the person's behalf, each as a call.
fn decisions() -> Vec<(&'static str, Value)> {
  vec![
    (
      "decide_approval",
      json!({ "project": "Kontur", "approval": "unknown", "decision": "deny" }),
    ),
    (
      "answer_question",
      json!({ "project": "Kontur", "run": "999", "text": "Keep it." }),
    ),
    (
      "decide_plan",
      json!({ "project": "Kontur", "run": "999", "decision": "request_changes", "note": "Keep it." }),
    ),
    continuing(&json!({ "kind": "allow_retry" })),
    continuing(&json!({ "kind": "return_to", "step": "implement", "note": "Again." })),
    continuing(&json!({ "kind": "accept" })),
    continuing(&json!({ "kind": "resolve_conflicts" })),
    continuing(&json!({ "kind": "raise_limits", "budget_tokens": 2_000_000, "time_limit_minutes": null })),
    continuing(&json!({ "kind": "raise_step_budget", "budget_tokens": null })),
    (
      "complete_run",
      json!({ "project": "Kontur", "run": "999", "anyway": true }),
    ),
  ]
}

#[test]
fn decisions_reach_person_domain_tools_wait_under_autopilot_until_the_person_lets_automation_answer() {
  let (mut app, seeded) = Served::seeded("task-states");
  autopilot(&seeded.repo);
  for (tool, args) in decisions() {
    assert_eq!(
      app.refused(tool, &args)["code"],
      "automation_not_allowed",
      "{tool} {args}"
    );
  }
  app
    .services()
    .workspace
    .set_automation_answers_approvals(true)
    .expect("consent");
  for (tool, args) in decisions() {
    assert_ne!(
      app.refused(tool, &args)["code"],
      "automation_not_allowed",
      "{tool} {args}"
    );
  }
}

#[test]
fn decisions_reach_person_orchestrator_answer_is_proposed_under_autopilot_until_its_switches() {
  let (mut served, seeded) = Served::seeded("cards");
  autopilot(&seeded.repo);
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let (_, run) = task_run(&db, "Pick where the docs button goes");
  let scope = chat(&served, &mut db);
  let answer = json!({ "project": "Kontur", "run": run.id().to_string(), "choices": ["In the top bar"] });
  let told = chat_call(&mut served, &scope, "answer_question", answer.clone()).expect("proposed");
  assert_eq!(told["waits_for_person"], true, "{told}");
  assert!(db.questions(run.id()).expect("questions")[0].waits());
  allow(&served, OrchestratorConsent::AnswerAgents);
  served
    .services()
    .workspace
    .set_automation_answers_approvals(true)
    .expect("consent");
  // Sent at once, as today: this session has no runner, so the runner's refusal comes back, never a proposal.
  let refused = chat_call(&mut served, &scope, "answer_question", answer).expect_err("no runner answers here");
  assert!(
    matches!(&refused, ToolError::Refused { code, .. } if code == "runner_unavailable"),
    "{refused:?}"
  );
}

#[test]
fn decisions_reach_person_orchestrator_proposes_how_a_waiting_run_goes_on_under_autopilot() {
  let (mut served, seeded) = Served::seeded("tasks");
  autopilot(&seeded.repo);
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let (_, run) = task_run(&db, "Cache GitHub API responses");
  let before = run.last_sequence();
  let scope = chat(&served, &mut db);
  served
    .services()
    .workspace
    .set_automation_answers_approvals(true)
    .expect("consent");
  for consent in [
    OrchestratorConsent::AnswerAgents,
    OrchestratorConsent::MergeRuns,
    OrchestratorConsent::ContinueInterrupted,
  ] {
    allow(&served, consent);
  }
  let retry = json!({ "project": "Kontur", "run": run.id().to_string(), "choice": { "kind": "allow_retry" } });
  let told = chat_call(&mut served, &scope, "continue_run", retry).expect("proposed");
  assert_eq!(told["waits_for_person"], true, "{told}");
  let raise = json!({
    "project": "Kontur", "run": run.id().to_string(),
    "choice": { "kind": "raise_limits", "budget_tokens": null, "time_limit_minutes": null },
  });
  let refused = chat_call(&mut served, &scope, "continue_run", raise).expect_err("the person's");
  assert!(
    matches!(&refused, ToolError::Refused { code, .. } if code == "person_only"),
    "{refused:?}"
  );
  let after = db.run(run.id()).expect("read").expect("the run").last_sequence();
  assert_eq!(after, before, "the run still waits for the person");
}
