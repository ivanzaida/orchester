//! `send_message` through Orchester's MCP server, with the real runner and the fake CLIs: the message
//! does what the Conversation tab's composer does in the run's state (ORC-79). It resumes a paused
//! run, follows up a run ready for review, runs a cancelled task again, continues a run that waits for
//! a decision from the step it stopped at, and waits for the person to raise the limit a run stopped
//! at; a blank message and one for an earlier run are refused.

use std::path::Path;

use orchester_core::{
  conversation::Delivery,
  resource::{FailureRoute, StepKey},
  run::{AttemptReason, PauseReason, Run, RunStatus},
  store::{ActivityStore, ConversationStore, RunStore, TaskStore},
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::{agent, ok, refused};
use crate::{
  harness::Desktop,
  mcp_agent::McpAgent,
  task_run::{agent_step, init_repository, programs, seed, seed_claude, seed_steps},
};

/// The latest run of task `key`, once it has one.
fn latest_of(repo: &Path, key: &str) -> Option<Run> {
  let db = ProjectDb::open(repo).expect("open the project");
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.key().to_string() == key)?;
  let latest = db.runs_of_task(task.id()).expect("runs").pop()?;
  Some(latest.expect("the latest run can be read"))
}

fn run_is(repo: &Path, key: &str, wanted: impl Fn(&Run) -> bool) -> bool {
  latest_of(repo, key).is_some_and(|run| wanted(&run))
}

/// The run's agent has started: its first invocation is recorded.
fn agent_at_work(repo: &Path, key: &str) -> bool {
  latest_of(repo, key).is_some_and(|run| {
    !ProjectDb::open(repo)
      .expect("open the project")
      .invocations(run.id())
      .expect("invocations")
      .is_empty()
  })
}

/// Where the person's messages to task `key` stand.
fn deliveries(repo: &Path, key: &str) -> Vec<Delivery> {
  let db = ProjectDb::open(repo).expect("open the project");
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.key().to_string() == key)
    .expect("the task");
  db.messages(task.id())
    .expect("messages")
    .iter()
    .filter_map(orchester_core::conversation::Message::delivery)
    .collect()
}

fn all_read(repo: &Path, key: &str) -> bool {
  let stored = deliveries(repo, key);
  !stored.is_empty()
    && stored
      .iter()
      .all(|delivery| matches!(delivery, Delivery::Acknowledged { .. }))
}

/// Creates task `title` on Delivery whose agent does what `brief` says, within `budget` tokens, and
/// runs it; its run's id.
fn create_and_run(agent: &mut McpAgent, (title, brief): (&str, &str), budget: u64) -> String {
  let created = ok(
    agent,
    "create_task",
    &json!({
      "project": "Kontur",
      "title": title,
      "brief": brief,
      "criteria": ["README.md has the agent's line"],
      "pipeline": "Delivery",
      "budget_tokens": budget,
    }),
  );
  let key = created["task"]["key"].as_str().expect("a key");
  let started = ok(agent, "run_task", &json!({"project": "Kontur", "key": key}));
  started["run"]["id"].as_str().expect("a run").to_owned()
}

fn message(run: &str, text: &str) -> Value {
  json!({"project": "Kontur", "run": run, "text": text})
}

/// Sends `text` to `run` and checks what the composer's action was and how it ended.
fn send(agent: &mut McpAgent, (run, text): (&str, &str), (action, outcome): (&str, &str)) -> Value {
  let sent = ok(agent, "send_message", &message(run, text));
  assert_eq!(sent["action"], action, "{sent}");
  assert_eq!(sent["outcome"], outcome, "{sent}");
  sent
}

#[test]
fn app_mcp_send_message_resumes_follows_up_and_runs_again_as_the_composer_does() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner_and_app_mcp(runner, init_repository, |repo| seed(repo, &fake, true));
  let repo = desktop.repo().to_path_buf();
  let mut agent = agent(&desktop);
  let run = create_and_run(&mut agent, ("Say hello", "Work slowly."), 1_400_000);
  desktop.wait_until("the agent works", |_| agent_at_work(&repo, "KON-1"));
  ok(&mut agent, "pause_run", &json!({"project": "Kontur", "run": run}));
  desktop.wait_until("the run paused", |_| {
    run_is(&repo, "KON-1", |run| matches!(run.status(), RunStatus::Paused(_)))
  });
  let blank = refused(&mut agent, "send_message", &message(&run, "  "));
  assert_eq!(blank["code"], "blank_message");
  send(&mut agent, (&run, "Check the OAuth dialog."), ("resume", "taken"));
  desktop.wait_until("the resumed run read it and succeeded", |_| {
    run_is(&repo, "KON-1", |run| run.status() == &RunStatus::Succeeded) && all_read(&repo, "KON-1")
  });
  send(&mut agent, (&run, "Add a second line."), ("follow_up", "taken"));
  desktop.wait_until("the follow-up read it", |_| {
    run_is(&repo, "KON-1", |run| {
      run.status() == &RunStatus::Succeeded
        && matches!(
          run.current_attempt().map(|attempt| attempt.reason()),
          Some(AttemptReason::FollowUp { .. })
        )
    }) && all_read(&repo, "KON-1")
  });

  let first = create_and_run(&mut agent, ("Say goodbye", "Wait for release."), 1_400_000);
  desktop.wait_until("the agent works", |_| agent_at_work(&repo, "KON-2"));
  ok(&mut agent, "cancel_run", &json!({"project": "Kontur", "run": first}));
  desktop.wait_until("the run cancelled", |_| {
    run_is(&repo, "KON-2", |run| run.status() == &RunStatus::Cancelled)
  });
  send(&mut agent, (&first, "Start over, briefly."), ("run_again", "taken"));
  desktop.wait_until("the task's next run", |_| {
    run_is(&repo, "KON-2", |run| run.id().to_string() != first)
  });
  // Messages go to the task's latest run.
  let earlier = refused(&mut agent, "send_message", &message(&first, "Hello?"));
  assert_eq!(earlier["code"], "not_latest_run");
  std::fs::write(repo.join(".orchester").join("fake-release"), "go").expect("release");
  desktop.wait_until("the next run read it", |_| {
    run_is(&repo, "KON-2", |run| run.status() == &RunStatus::Succeeded) && all_read(&repo, "KON-2")
  });
}

#[test]
fn app_mcp_send_message_continues_a_run_waiting_for_a_decision() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner_and_app_mcp(runner, init_repository, |repo| {
    seed_steps(repo, &fake, |builder| {
      let mut review = agent_step(builder, "review", "Review", "Report failure.", &["implement"]);
      review.on_failure = Some(FailureRoute {
        return_to: StepKey::new("implement").expect("key"),
        max_retries: std::num::NonZeroU32::MIN,
      });
      vec![
        agent_step(builder, "implement", "Implement", "Append one line.", &[]),
        review,
      ]
    });
  });
  let repo = desktop.repo().to_path_buf();
  let mut agent = agent(&desktop);
  let run = create_and_run(&mut agent, ("Say hello", "Say hello."), 1_400_000);
  desktop.wait_until("the run waits for a decision", |_| {
    run_is(&repo, "KON-1", |run| {
      matches!(run.status(), RunStatus::WaitingForPerson(_))
    })
  });
  send(&mut agent, (&run, "So what do you suggest?"), ("continue", "taken"));
  desktop.wait_until("the continued run read it and succeeded", |_| {
    run_is(&repo, "KON-1", |run| {
      run.status() == &RunStatus::Succeeded && run.attempts().len() == 3
    }) && all_read(&repo, "KON-1")
  });
}

#[test]
fn app_mcp_send_message_waits_for_the_raised_limit_of_a_stopped_run() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner_and_app_mcp(runner, init_repository, |repo| seed_claude(repo, &fake));
  let repo = desktop.repo().to_path_buf();
  let mut agent = agent(&desktop);
  let run = create_and_run(&mut agent, ("Say hello", "Work in turns."), 5_000);
  desktop.wait_until("the run stopped at its limit", |_| {
    run_is(&repo, "KON-1", |run| {
      matches!(run.status(), RunStatus::Paused(PauseReason::LimitReached(_)))
    })
  });
  let sent = send(&mut agent, (&run, "Then add a test."), ("raise_limit", "awaits_limit"));
  assert_eq!(
    sent["reasons"][0],
    "The run stopped at its limit. Raise the limit to continue."
  );
  assert_eq!(deliveries(&repo, "KON-1"), [Delivery::Queued]);
}
