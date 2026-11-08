//! The run controls and the reads of what waits on a person, for a team task's run that has no row of its task here
//! (team authority P7): a paused run resumes under the claim its work started with, a fenced one and one that was
//! interrupted are refused before anything is stored, and the waits of a team run are listed under its recorded key.

use orchester_core::{
  run::{InterruptCause, WaitReason},
  store::TaskStore,
};
use serde_json::json;

use crate::team_runs::{Started, started};

fn paused() -> Started {
  let team = started();
  team.settle(|run, now| run.pause(now).expect("paused"));
  team
}

fn interrupted() -> Started {
  let team = started();
  team.settle(|run, now| run.interrupt(InterruptCause::RunnerLost, now).expect("interrupted"));
  team
}

fn no_task_rows(team: &Started) -> bool {
  team.checkout.db().tasks().expect("tasks").is_empty()
}

#[test]
fn team_mcp_a_paused_team_run_resumes_under_its_own_claim() {
  let team = paused();
  let resume = json!({"project": "Kontur", "run": team.run.to_string()});

  // The session has no runner, so the resume reaches the runner's refusal: no claim of the old sharing stream was asked
  // for (it would have cached the task's row here).
  assert_eq!(team.checkout.refused("resume_run", &resume), "runner_unavailable");
  assert!(no_task_rows(&team));
}

#[test]
fn team_mcp_a_fenced_team_run_is_not_resumed() {
  let team = paused();
  team.fence();
  let resume = json!({"project": "Kontur", "run": team.run.to_string()});

  assert_eq!(team.checkout.refused("resume_run", &resume), "claim_fenced");
  let message = json!({"project": "Kontur", "run": team.run.to_string(), "text": "Go on."});
  assert_eq!(team.checkout.refused("send_message", &message), "claim_fenced");
  assert_eq!(team.messages(), 0, "a refused message is not stored");
  assert!(no_task_rows(&team));
}

#[test]
fn team_mcp_a_fenced_run_that_was_interrupted_is_refused_as_fenced() {
  let team = interrupted();
  team.fence();

  let resume = json!({"project": "Kontur", "run": team.run.to_string()});
  assert_eq!(team.checkout.refused("resume_run", &resume), "claim_fenced");
}

#[test]
fn team_mcp_a_message_to_an_interrupted_team_run_is_refused_before_it_is_stored() {
  let team = interrupted();
  let message = json!({"project": "Kontur", "run": team.run.to_string(), "text": "Carry on."});

  // The fixture's run has no worktree, so the composer runs the task again with the message, which a team task does
  // only through the team server.
  assert_eq!(
    team.checkout.refused("send_message", &message),
    "team_run_again_unsupported"
  );
  assert_eq!(team.messages(), 0, "nothing was stored");
  assert!(no_task_rows(&team));
}

#[test]
fn team_mcp_a_message_to_a_team_run_caches_no_task_row_and_takes_no_claim() {
  let team = started();
  let message = json!({"project": "Kontur", "run": team.run.to_string(), "text": "Use the webhooks module."});

  team.checkout.call("send_message", &message).expect("sent");
  assert_eq!(team.messages(), 1);
  // The old sharing stream's claim would have cached the task's row here.
  assert!(no_task_rows(&team));
}

#[test]
fn team_mcp_list_questions_lists_the_message_wait_of_a_team_run() {
  let team = started();
  team.step_into(|run, execution, now| run.wait_step(execution, WaitReason::Message, now).expect("waits"));

  let listed = team
    .checkout
    .call("list_questions", &json!({"project": "Kontur"}))
    .expect("listed");
  let waits = listed["waiting_for_message"].as_array().expect("waits");
  assert_eq!(waits.len(), 1, "{listed}");
  assert_eq!(waits[0]["task"], "KON-7", "{listed}");
  assert_eq!(waits[0]["run"]["id"], team.run.to_string().as_str());
}

#[test]
fn team_mcp_get_changes_does_not_call_a_team_run_missing() {
  let team = started();
  let changes = team.checkout.call(
    "get_changes",
    &json!({"project": "Kontur", "run": team.run.to_string()}),
  );
  // The run has no worktree here, so the answer may be a refusal, but not that the run or its task is missing.
  if let Err(error) = changes {
    let code = error["error"]["code"].as_str().expect("a code");
    assert!(
      !code.ends_with("_not_found"),
      "a team run that exists is not missing: {error}"
    );
  }
}

#[test]
fn team_mcp_hold_queue_of_the_whole_project_answers_with_a_team_run_in_it() {
  let team = started();
  let project = json!({"project": "Kontur"});

  let held = team.checkout.call("hold_queue", &project).expect("held");
  assert_eq!(held["held"]["project"], true, "{held}");
  team.checkout.call("release_queue", &project).expect("released");
}
