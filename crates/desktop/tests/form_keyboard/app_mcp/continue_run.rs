//! `continue_run` on the real runner and the fake Claude, for each stop the task page continues: a run
//! stopped at its token budget continues with the budget and time limit the caller gives; one stopped
//! at a step's budget with that step's new budget; one stopped by the daily budget resumes once the
//! budget allows; and one whose step used up its retries gets one more retry or returns to an earlier
//! step with a note. A choice the page does not offer, a limit not above what the run used, and a run
//! not stopped are refused. Run with `cargo test -p orchester-desktop --test form_keyboard
//! app_mcp_continue_run`.

use orchester_core::{
  connection::AgentProvider,
  resource::Step,
  run::{PauseReason, RunLimit, RunStatus, Stuck},
  tokens::TokenBudget,
};
use serde_json::{Value, json};

use super::{agent, ok, refused};
use crate::{
  harness::Desktop,
  lifecycle::save_daily_budget,
  mcp_agent::McpAgent,
  task_graph::retry,
  task_run::{
    agent_step, create_and_run, fake_claude, has, init_repository, latest_run, programs, run_finished, seed_agent,
    seed_claude, seed_steps,
  },
};

/// `continue_run`'s arguments for run #1 with `choice`.
fn with(choice: Value) -> Value {
  let mut arguments = json!({"project": "Kontur", "run": "#1"});
  arguments["choice"] = choice;
  arguments
}

fn refused_with(client: &mut McpAgent, choice: Value) -> Value {
  refused(client, "continue_run", &with(choice))["code"].clone()
}

#[test]
fn app_mcp_continue_run_raises_the_limits_of_a_run_stopped_at_them() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner_and_app_mcp(runner, init_repository, |repo| seed_claude(repo, &fake));
  create_and_run(&mut desktop, "Work in turns.", "5000");
  desktop.wait_until("the raise-limit panel", |desktop| has(desktop, "raise-limit"));
  let mut client = agent(&desktop);

  assert_eq!(
    refused_with(&mut client, json!({"kind": "allow_retry"})),
    "choice_not_offered"
  );
  assert_eq!(
    refused_with(
      &mut client,
      json!({"kind": "raise_limits", "budget_tokens": 5000, "time_limit_minutes": 60})
    ),
    "invalid_limit"
  );
  assert_eq!(
    refused_with(&mut client, json!({"kind": "raise_limits"})),
    "invalid_arguments",
    "nothing defaults to a higher limit"
  );

  let continued = ok(
    &mut client,
    "continue_run",
    &with(json!({"kind": "raise_limits", "budget_tokens": 100_000, "time_limit_minutes": 60})),
  );
  assert_ne!(continued["run"]["status"]["state"], "paused", "{continued}");
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
  assert_eq!(
    refused_with(
      &mut client,
      json!({"kind": "raise_limits", "budget_tokens": 200_000, "time_limit_minutes": 60})
    ),
    "not_stopped_at_limit"
  );
}

#[test]
fn app_mcp_continue_run_raises_the_budget_of_the_step_it_stopped_at() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner_and_app_mcp(runner, init_repository, |repo| {
    seed_agent(repo, &fake_claude(&fake), AgentProvider::ClaudeCodeCli, |builder| {
      let implement = agent_step(builder, "implement", "Implement", "Work in turns.", &[]);
      vec![Step {
        budget: Some(TokenBudget::new(1_000).expect("above zero")),
        ..implement
      }]
    });
  });
  create_and_run(&mut desktop, "Say hello.", "1400000");
  desktop.wait_until("the raise-limit panel", |desktop| has(desktop, "raise-limit"));
  let mut client = agent(&desktop);

  assert_eq!(
    refused_with(
      &mut client,
      json!({"kind": "raise_limits", "budget_tokens": 2_000_000, "time_limit_minutes": 60})
    ),
    "choice_not_offered"
  );
  assert_eq!(
    refused_with(&mut client, json!({"kind": "raise_step_budget", "budget_tokens": 1000})),
    "invalid_limit"
  );
  assert_eq!(
    refused_with(&mut client, json!({"kind": "raise_step_budget"})),
    "invalid_arguments",
    "nothing lifts a limit by itself"
  );
  // `null` lifts the step's budget for this run.
  ok(
    &mut client,
    "continue_run",
    &with(json!({"kind": "raise_step_budget", "budget_tokens": null})),
  );
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
}

#[test]
fn app_mcp_continue_run_resumes_a_run_the_daily_budget_stopped_once_it_allows() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner_and_app_mcp(runner, init_repository, |repo| {
    seed_claude(repo, &fake);
    save_daily_budget(repo, 5_000);
  });
  create_and_run(&mut desktop, "Work in turns.", "1400000");
  desktop.wait_until("the daily budget stop", |desktop| {
    latest_run(desktop.repo())
      .is_some_and(|run| run.status() == &RunStatus::Paused(PauseReason::LimitReached(RunLimit::DailyBudget)))
  });
  let mut client = agent(&desktop);

  assert_eq!(
    refused_with(
      &mut client,
      json!({"kind": "raise_limits", "budget_tokens": 2_000_000, "time_limit_minutes": 60})
    ),
    "choice_not_offered"
  );
  // Today's budget is still used up: the runner refuses, as it refuses the page's Resume.
  refused(&mut client, "continue_run", &with(json!({"kind": "resume"})));
  save_daily_budget(desktop.repo(), 20_000_000);
  ok(&mut client, "continue_run", &with(json!({"kind": "resume"})));
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
}

#[test]
fn app_mcp_continue_run_retries_a_step_or_returns_to_an_earlier_one() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner_and_app_mcp(runner, init_repository, |repo| {
    seed_steps(repo, &fake, |builder| {
      let mut review = agent_step(builder, "review", "Review", "Report failure.", &["implement"]);
      review.on_failure = retry("implement");
      vec![
        agent_step(builder, "implement", "Implement", "Write own file.", &[]),
        review,
      ]
    });
  });
  create_and_run(&mut desktop, "Say hello.", "1400000");
  // Waiting for a person after more than `attempts` attempts.
  let waiting = |attempts: usize| {
    move |desktop: &mut Desktop| {
      latest_run(desktop.repo()).is_some_and(|run| {
        matches!(run.status(), RunStatus::WaitingForPerson(Stuck::RetriesExhausted(_)))
          && run.attempts().len() > attempts
      })
    }
  };
  desktop.wait_until("the run to wait for a person", waiting(1));
  let mut client = agent(&desktop);

  assert_eq!(
    refused_with(
      &mut client,
      json!({"kind": "raise_limits", "budget_tokens": 2_000_000, "time_limit_minutes": 60})
    ),
    "choice_not_offered"
  );
  ok(&mut client, "continue_run", &with(json!({"kind": "allow_retry"})));
  desktop.wait_until("one more attempt, then waiting again", waiting(2));

  assert_eq!(
    refused_with(
      &mut client,
      json!({"kind": "return_to", "step": "Deploy", "note": "Try again."})
    ),
    "step_not_offered"
  );
  assert_eq!(
    refused_with(
      &mut client,
      json!({"kind": "return_to", "step": "implement", "note": "  "})
    ),
    "invalid_arguments"
  );
  ok(
    &mut client,
    "continue_run",
    &with(json!({"kind": "return_to", "step": "Implement", "note": "Write the file again."})),
  );
  desktop.wait_until("the return to Implement, then waiting again", waiting(3));
}
