//! A run's own model and effort for a step (ORC-322), chosen while the step's agent is stopped at a limit: its next
//! turn starts with them, at the CLI's arguments for Claude Code (`--model`, `--effort`) and in what the runner tells
//! Codex App Server (`thread/resume`'s model, `turn/start`'s effort), continuing the same session; the invocation
//! records the model it ran. End to end against `orchester-fake-claude` and `orchester-fake-codex`. Needs `git`.
//! Run with `cargo test -p orchester-runner --features test-agent --test run_model run_model`.

mod support;

use std::time::Duration;

use orchester_core::{
  connection::AgentProvider,
  id::{IdempotencyKey, RunId},
  resource::{ModelId, StepKey},
  run::{Answerer, ModelChoice, ModelEvent, ModelEventKind, ModelScope, RaisedLimits, RunStatus},
  store::ActivityStore,
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;

use self::support::{Setup, project, reported, scheduler, threads, wait_for};

fn raised() -> RaisedLimits {
  RaisedLimits {
    budget: Some(TokenBudget::new(2_000_000).expect("above zero")),
    max_duration: Some(Duration::from_secs(30 * 60)),
  }
}

/// Chooses `model` and `effort` for the run's Implement, as a person's change from its Conversation stores them.
fn choose(root: &std::path::Path, run: RunId, (model, effort): (&str, &str)) {
  let implement = StepKey::new("implement").expect("key");
  let chosen = ModelChoice {
    model: ModelId::new(model).expect("model"),
    effort: Some(effort.to_owned()),
  };
  let event = ModelEvent {
    at: Timestamp::now(),
    by: Answerer::Person,
    step: implement.clone(),
    kind: ModelEventKind::Changed {
      scope: ModelScope::Step,
      to: chosen.clone(),
      was: chosen.clone(),
      also: Vec::new(),
      before: Vec::new(),
    },
  };
  ProjectDb::open(root)
    .expect("open")
    .change_step_models(run, &[(implement, Some(chosen))], &event)
    .expect("chosen");
}

async fn start(scheduler: &Scheduler, task: orchester_core::id::TaskId) -> RunId {
  scheduler
    .start_run(IdempotencyKey::generate(), task)
    .await
    .expect("the run starts")
}

#[tokio::test(flavor = "multi_thread")]
async fn run_model_chosen_for_claude_starts_its_next_turn_with_the_model_and_effort() {
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    model: Some("claude-opus-5-5"),
    budget: Some(5_000),
    ..Setup::new("Work in turns.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = start(&scheduler, project.task).await;
  wait_for(root, &mut events, id, |run| {
    matches!(run.status(), RunStatus::Paused(_))
  })
  .await;
  choose(root, id, ("claude-fable-5-1", "high"));
  scheduler.raise_limit(id, raised()).await.expect("the run continues");
  let done = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  let argv = reported(&db, id, "argv: ");
  assert_eq!(argv.len(), 2);
  assert!(
    argv[0].contains(" --model claude-opus-5-5 --effort low "),
    "{}",
    argv[0]
  );
  assert!(
    argv[1].contains(" --model claude-fable-5-1 --effort high "),
    "{}",
    argv[1]
  );
  let session = db.invocations(id).expect("invocations")[0]
    .session
    .clone()
    .expect("the CLI reported its session");
  assert!(argv[1].ends_with(&format!("--resume {session}")), "{}", argv[1]);
  let models: Vec<String> = db
    .invocations(id)
    .expect("invocations")
    .iter()
    .map(|invocation| invocation.model().expect("an agent invocation").as_str().to_owned())
    .collect();
  assert_eq!(models, ["claude-opus-5-5", "claude-fable-5-1"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn run_model_chosen_for_codex_resumes_its_thread_with_the_model_and_effort() {
  let project = project(&Setup {
    max_duration: Some(Duration::from_secs(2)),
    ..Setup::new("Keep working.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = start(&scheduler, project.task).await;
  wait_for(root, &mut events, id, |run| {
    matches!(run.status(), RunStatus::Paused(_))
  })
  .await;
  choose(root, id, ("gpt-5.5-codex", "xhigh"));
  // The time the run spent stopped does not count: the raised limit is measured against the agent's time.
  tokio::time::sleep(Duration::from_secs(2)).await;
  scheduler.raise_limit(id, raised()).await.expect("the run continues");
  let done = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  let threads = threads(&db, id);
  assert_eq!(threads.len(), 2);
  assert_ne!(threads[0]["model"], "gpt-5.5-codex");
  assert_eq!(threads[1]["threadId"], "fake-thread-1", "the session continues");
  assert_eq!(threads[1]["model"], "gpt-5.5-codex");
  let turns: Vec<serde_json::Value> = reported(&db, id, "turn: ")
    .iter()
    .map(|turn| serde_json::from_str(turn).expect("the fake reports JSON"))
    .collect();
  assert_eq!(turns.last().expect("a turn")["effort"], "xhigh");
  let last = db.invocations(id).expect("invocations").pop().expect("an invocation");
  assert_eq!(last.model().map(ModelId::as_str), Some("gpt-5.5-codex"));
}
