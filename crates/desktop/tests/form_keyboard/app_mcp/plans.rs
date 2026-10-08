//! Plan decisions through Orchester's MCP server, with the real runner and `orchester-fake-codex`: a gated
//! Plan step, whose agent works until the test releases it, proposes plan v1 and waits. `get_run` and
//! `list_approvals` show the waiting plan; `decide_plan` refuses what the plan card would not take (no plan
//! waiting, a blank note, a note with an approval, a plan decided already, a version that no longer waits) and decides
//! through the runner as the card does: approving continues the run to Implement, requesting changes resumes the Plan
//! step's agent with the note. An open task page follows the decision in place. Run with
//! `cargo test -p orchester-desktop --test form_keyboard app_mcp_decide_plan`.

use std::path::Path;

use orchester_core::{
  resource::Step,
  run::{Plan, PlanDecision, ReviewNote, RunStatus},
  store::{ActivityStore, RunStore},
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::{agent, ok, refused};
use crate::{
  conversation::{holds, release},
  conversation_composer::write,
  github::shows,
  harness::Desktop,
  task_run::{agent_step, create_and_run, has, init_repository, latest_run, programs, run_finished, seed_steps},
};

/// Plan (gated), then Implement, with Orchester's MCP server; the task page of a run whose Plan agent
/// works until the test releases it is open.
fn gated() -> Desktop {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner_and_app_mcp(runner, init_repository, |repo| {
    seed_steps(repo, &fake, |builder| {
      vec![
        Step {
          approval_gate: true,
          ..agent_step(builder, "plan", "Plan", "Propose a plan.", &[])
        },
        agent_step(builder, "implement", "Implement", "Follow the plan.", &["plan"]),
      ]
    });
  });
  create_and_run(&mut desktop, "Wait for release.", "1400000");
  desktop
}

fn decide(decision: &str, note: Option<&str>) -> Value {
  let mut arguments = json!({"project": "Kontur", "run": "1", "decision": decision});
  if let Some(note) = note {
    arguments["note"] = json!(note);
  }
  arguments
}

/// [`decide`] for plan `version` only.
fn decide_version(decision: &str, version: u32) -> Value {
  let mut arguments = decide(decision, None);
  arguments["version"] = json!(version);
  arguments
}

fn get_run() -> Value {
  json!({"project": "Kontur", "run": "1"})
}

/// The plans of the project's only run, as stored.
fn plans(repo: &Path) -> Vec<Plan> {
  let run = latest_run(repo).expect("the run");
  ProjectDb::open(repo).expect("open").plans(run.id()).expect("plans")
}

/// Waits until plan `version` waits for a decision.
fn plan_waits(desktop: &mut Desktop, version: u32) {
  desktop.wait_until(&format!("plan v{version} waits"), |desktop| {
    let repo = desktop.repo();
    latest_run(repo).is_some_and(|run| run.waits_for_a_person())
      && plans(repo)
        .last()
        .is_some_and(|plan| plan.version.get() == version && plan.review.is_none())
  });
}

/// The lines the run's agent reported with `prefix`.
fn reported(repo: &Path, prefix: &str) -> Vec<String> {
  let run = latest_run(repo).expect("the run");
  ProjectDb::open(repo)
    .expect("open")
    .activity(run.id(), 0)
    .expect("activity")
    .iter()
    .filter_map(|entry| match &entry.event {
      orchester_core::connection::stream::AgentEvent::Raw { line, .. } => line.strip_prefix(prefix).map(str::to_owned),
      _ => None,
    })
    .collect()
}

#[test]
fn app_mcp_decide_plan_approves_the_waiting_plan_and_the_run_continues() {
  let mut desktop = gated();
  desktop.wait_until("the Plan agent works", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| run.status() == &RunStatus::Running)
  });
  let mut client = agent(&desktop);
  let none = refused(&mut client, "decide_plan", &decide("approve", None));
  assert_eq!(none["code"], "no_plan_waiting", "{none}");
  assert_eq!(none["reasons"][0], "No plan waits for a decision in run #1.");
  assert_eq!(ok(&mut client, "get_run", &get_run())["awaiting"]["plan"], Value::Null);

  release(&desktop);
  plan_waits(&mut desktop, 1);
  let run = ok(&mut client, "get_run", &get_run());
  let plan = &run["awaiting"]["plan"];
  assert_eq!(plan["version"], 1, "{run}");
  assert_eq!(plan["step"], json!({"key": "plan", "name": "Plan"}));
  assert_eq!(plan["agent"], "Builder");
  assert!(
    plan["excerpt"]
      .as_str()
      .is_some_and(|text| text.contains("Finished Plan")),
    "{plan}"
  );
  assert_eq!(run["awaiting"]["plan_decisions"], 1);
  let listed = ok(&mut client, "list_approvals", &json!({"project": "Kontur"}));
  let waiting = &listed["plans"][0];
  assert_eq!(waiting["version"], 1, "{listed}");
  assert_eq!(waiting["task"], "KON-1");
  assert_eq!(waiting["run"]["number"], 1);

  for (arguments, code) in [
    (decide("request_changes", None), "blank_note"),
    (decide("request_changes", Some("   ")), "blank_note"),
    (decide("approve", Some("Looks good.")), "note_not_taken"),
    (
      json!({"project": "Kontur", "run": "#9", "decision": "approve"}),
      "run_not_found",
    ),
  ] {
    let error = refused(&mut client, "decide_plan", &arguments);
    assert_eq!(error["code"], code, "{arguments}: {error}");
  }
  assert_eq!(plans(desktop.repo())[0].review, None, "a refusal decides nothing");

  let decided = ok(&mut client, "decide_plan", &decide_version("approve", 1));
  assert_eq!(
    decided["plan"],
    json!({"version": 1, "step": {"key": "plan", "name": "Plan"}, "decided": "approved"})
  );
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
  let run = ok(&mut client, "get_run", &get_run());
  assert_eq!(
    run["steps"][1]["status"]["state"], "passed",
    "Implement ran after the approved plan: {run}"
  );
  assert_eq!(run["awaiting"]["plan"], Value::Null);
  let twice = refused(&mut client, "decide_plan", &decide("approve", None));
  assert_eq!(twice["code"], "plan_already_decided", "{twice}");
  assert_eq!(
    twice["reasons"][0],
    "Plan v1 of Plan was approved already; no plan waits for a decision."
  );
  // The open task page followed: the review left the Summary, which shows the finished run.
  desktop.wait_until("the page shows the finished run", |desktop| {
    has(desktop, "task-page") && !has(desktop, "plan-review") && has(desktop, "task-ready-notice")
  });
}

#[test]
fn app_mcp_decide_plan_requests_changes_and_the_open_page_follows_in_place() {
  let mut desktop = gated();
  release(&desktop);
  plan_waits(&mut desktop, 1);
  desktop.click("task-tab-conversation");
  desktop.wait_until("the plan's card", |desktop| has(desktop, "conversation-approve-plan"));
  write(&mut desktop, "Draft");
  let mut client = agent(&desktop);

  let decided = ok(
    &mut client,
    "decide_plan",
    &decide("request_changes", Some("  Keep email sign-in. ")),
  );
  assert_eq!(decided["plan"]["version"], 1, "{decided}");
  assert_eq!(decided["plan"]["decided"], "changes_requested");
  plan_waits(&mut desktop, 2);
  desktop.wait_until("the page shows plan v2", |desktop| {
    holds(desktop, "conversation", "Plan v2 · Say hello") && shows(desktop, "conversation", "Changes requested")
  });
  // In place: the Conversation tab stays, with the person's focus and draft.
  assert_eq!(desktop.focused().as_deref(), Some("conversation-input"));
  assert_eq!(desktop.value("conversation-input"), "Draft");

  let [first, second] = <[Plan; 2]>::try_from(plans(desktop.repo())).expect("two plans");
  let note = ReviewNote::new("Keep email sign-in.").expect("a note");
  assert_eq!(first.decision(), Some(&PlanDecision::ChangesRequested { note }));
  assert_eq!((second.execution, second.review), (first.execution, None));
  // The Plan step's agent resumed with the note.
  let prompt = reported(desktop.repo(), "prompt| ");
  assert!(
    prompt.iter().any(|line| line == "What should change in plan v1:"),
    "{prompt:?}"
  );
  assert!(prompt.iter().any(|line| line == "Keep email sign-in."), "{prompt:?}");
  assert_eq!(ok(&mut client, "get_run", &get_run())["awaiting"]["plan"]["version"], 2);

  // A decision meant for v1 does not decide v2, which the client has not read.
  let changed = refused(&mut client, "decide_plan", &decide_version("approve", 1));
  assert_eq!(changed["code"], "plan_version_changed", "{changed}");
  assert_eq!(
    changed["reasons"][0],
    "Plan v1 no longer waits for a decision; plan v2 of Plan waits now."
  );
  assert_eq!(plans(desktop.repo())[1].review, None, "v2 still waits");
  ok(&mut client, "decide_plan", &decide_version("approve", 2));
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
}
