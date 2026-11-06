//! The executor reaches its coordinator only through values that serialize (`docs/architecture/remote-runners.md`,
//! rule 4). Each of the runner's main run scenarios runs twice on a project of its own: once in process, and once on
//! a scheduler whose runs' executor sends every coordinator-link request, sink record and computer's resource request
//! across a channel as JSON and goes on with the answer read back from JSON (`Scheduler::over_channel`). Both leave
//! the same execution rows (`support::rows`: every row, ids, revisions, times and the project's folder left out).
//! The scenarios: a run that proposes a plan, takes a message, asks an approval and a question, checkpoints and
//! succeeds; Run again after a failed run; a worktree that cannot be created; a cancelled run. Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test serialized_boundary serialized_boundary`.

mod support;

use std::{collections::BTreeMap, fs, path::Path};

use orchester_core::{
  connection::{AgentProvider, stream::AgentEvent},
  conversation::{Message, MessageBody, Recipient, Timing},
  id::{IdempotencyKey, RunId, TaskId},
  question::{AskedQuestions, GivenAnswer},
  run::{Answerer, ApprovalAnswer, PlanDecision, Run, RunStatus},
  store::{ActivityStore, ConversationStore, RunStore},
  time::Timestamp,
};
use orchester_ipc::{Event, Rejection};
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tempfile::TempDir;
use tokio::sync::mpsc;

use self::support::{
  PATIENCE, Setup, StepSpec, TestProject, pending_approval, project, reported, rows::Rows, scheduler_with, wait_for,
  wait_for_idle,
};

/// How a scenario's executor reaches its coordinator.
#[derive(Clone, Copy, Debug)]
enum Way {
  InProcess,
  OverChannel,
}

/// What a scenario left: the project's execution rows and what crossed the channel. The project's folder is kept
/// until the comparison is done.
struct Left {
  rows: Rows,
  sent: BTreeMap<String, usize>,
  _repo: TempDir,
}

fn scheduler_for(project: &TestProject, way: Way) -> (Scheduler, mpsc::UnboundedReceiver<Event>) {
  let (scheduler, events) = scheduler_with(project.repo.path(), project.vault.clone());
  match way {
    Way::InProcess => (scheduler, events),
    Way::OverChannel => (scheduler.over_channel(), events),
  }
}

async fn left(project: TestProject, scheduler: &Scheduler) -> Left {
  wait_for_idle(scheduler).await;
  Left {
    rows: Rows::of(project.repo.path()),
    sent: scheduler.sent_over_channel(),
    _repo: project.repo,
  }
}

/// Runs `scenario` in process and over the channel, and checks that both leave the same rows, that nothing crossed in
/// process and that each of `crossed` (the names of the link's, the sink's or the computer's methods) crossed the
/// channel.
async fn same_rows<F>(scenario: impl Fn(Way) -> F, crossed: &[&str]) -> Rows
where
  F: Future<Output = Left>,
{
  let in_process = scenario(Way::InProcess).await;
  let over_channel = scenario(Way::OverChannel).await;
  assert!(in_process.sent.is_empty(), "nothing crosses in process");
  for method in crossed {
    assert!(
      over_channel.sent.contains_key(*method),
      "`{method}` crossed the channel: {:?}",
      over_channel.sent
    );
  }
  let differences = in_process.rows.differences(&over_channel.rows);
  assert!(
    differences.is_empty(),
    "the run over the channel leaves other rows (- in process, + over the channel):\n{differences}"
  );
  in_process.rows
}

async fn start(scheduler: &Scheduler, task: TaskId) -> RunId {
  scheduler
    .start_run(IdempotencyKey::generate(), task)
    .await
    .expect("the run starts")
}

fn ended(run: &Run) -> bool {
  run.status().is_terminal()
}

/// Plan (gated), then a step that takes a message, one that asks to run a command and one that asks a question.
fn decisions() -> Setup {
  Setup {
    provider: AgentProvider::ClaudeCodeCli,
    steps: vec![
      StepSpec::new("plan", "Plan", "Propose a plan.", &[]).gate(),
      StepSpec::new("message", "Message", "Take a message. Wait for a message.", &["plan"]),
      StepSpec::new(
        "approval",
        "Approval",
        "Take an approval. Run `pnpm build`.",
        &["message"],
      ),
      StepSpec::new(
        "question",
        "Question",
        "Take a question. Ask with options, several allowed.",
        &["approval"],
      ),
    ],
    ..Setup::new("Say hello.")
  }
}

/// Sends a person's message to `run` now, as the desktop does, and tells the scheduler.
async fn send_now(root: &Path, scheduler: &Scheduler, (task, run): (TaskId, RunId)) {
  let message = Message::from_person(
    task,
    Some(run),
    Recipient::Orchestrator,
    MessageBody::new("Also cover the cancelled dialog.").expect("a message"),
    Timestamp::now(),
  )
  .sent(Timing::Now)
  .expect("a person's message");
  ProjectDb::open(root)
    .expect("open")
    .send_message(IdempotencyKey::generate(), &message)
    .expect("stored");
  scheduler.deliver_messages(run).await.expect("taken");
}

/// What `find` reads in the project at `root` once it reads anything, read again at each of the scheduler's
/// notifications.
async fn waited<T>(
  root: &Path,
  events: &mut mpsc::UnboundedReceiver<Event>,
  what: &str,
  find: impl Fn(&ProjectDb) -> Option<T>,
) -> T {
  let found = tokio::time::timeout(PATIENCE, async {
    loop {
      if let Some(found) = find(&ProjectDb::open(root).expect("open")) {
        return found;
      }
      events.recv().await.expect("the scheduler notifies");
    }
  })
  .await;
  found.unwrap_or_else(|_| panic!("{what} in time"))
}

/// Whether the agent of `run` started `command`.
fn started(db: &ProjectDb, run: RunId, command: &str) -> Option<()> {
  let activity = db.activity(run, 0).expect("activity");
  activity
    .iter()
    .any(|entry| matches!(&entry.event, AgentEvent::CommandStarted { command: started, .. } if started == command))
    .then_some(())
}

async fn decided(way: Way) -> Left {
  let project = project(&decisions());
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_for(&project, way);
  let id = start(&scheduler, project.task).await;
  wait_for(root, &mut events, id, Run::waits_for_a_person).await;
  let plan = ProjectDb::open(root)
    .expect("open")
    .plans(id)
    .expect("plans")
    .pop()
    .expect("a plan");
  scheduler
    .decide_plan(
      id,
      plan.execution,
      plan.version,
      PlanDecision::Approved,
      Answerer::Person,
    )
    .await
    .expect("the plan is approved");
  waited(root, &mut events, "the message step runs its command", |db| {
    started(db, id, "pnpm test")
  })
  .await;
  send_now(root, &scheduler, (project.task, id)).await;
  let request = pending_approval(root, &mut events, id).await;
  scheduler
    .decide_tool_approval(id, request.id, ApprovalAnswer::Once, Answerer::Person)
    .await
    .expect("approved");
  let asked = waited(root, &mut events, "a question waits", |db| {
    let questions = db.questions(id).expect("questions");
    questions.into_iter().find(AskedQuestions::waits)
  })
  .await;
  let answer = vec![GivenAnswer {
    choices: vec!["Under Sign in".to_owned()],
    text: None,
  }];
  scheduler
    .answer_question(id, asked.request.id().to_owned(), answer, Answerer::Person)
    .await
    .expect("answered");
  let run = wait_for(root, &mut events, id, ended).await;
  assert_eq!(run.status(), &RunStatus::Succeeded, "{way:?}");
  left(project, &scheduler).await
}

#[tokio::test(flavor = "multi_thread")]
async fn serialized_boundary_a_run_with_a_plan_a_message_an_approval_and_a_question_leaves_the_same_rows() {
  let rows = same_rows(
    decided,
    &[
      "run_log",
      "invocation_start",
      "activity",
      "invocation",
      "message",
      "approval",
      "question",
      "messages",
      "pending_approvals",
      "connection",
    ],
  )
  .await;
  for table in ["plans", "messages", "tool_approvals", "questions", "checkpoints"] {
    assert!(!rows.table(table).is_empty(), "the scenario leaves {table}");
  }
}

async fn ran_again(way: Way) -> Left {
  let project = project(&Setup::new("Report failure."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_for(&project, way);
  let first = start(&scheduler, project.task).await;
  let failed = wait_for(root, &mut events, first, ended).await;
  assert!(matches!(failed.status(), RunStatus::Failed(_)), "{way:?}");
  let again = scheduler
    .run_again(IdempotencyKey::generate(), project.task, first, None)
    .await
    .expect("the task runs again");
  wait_for(root, &mut events, again, ended).await;
  left(project, &scheduler).await
}

#[tokio::test(flavor = "multi_thread")]
async fn serialized_boundary_run_again_after_a_failed_run_leaves_the_same_rows() {
  let rows = same_rows(ran_again, &["run_log", "invocation_start", "activity", "connection"]).await;
  assert!(!rows.table("runs").is_empty());
}

async fn worktree_failed(way: Way) -> Left {
  let project = project(&Setup::new("Report failure."));
  let root = project.repo.path();
  // Git would show a worktree in the person's checkout: the runner refuses to create it there.
  fs::write(root.join(".git/info/exclude"), "").expect("stop excluding .orchester");
  let (scheduler, mut events) = scheduler_for(&project, way);
  let refused = scheduler.start_run(IdempotencyKey::generate(), project.task).await;
  let Err(Rejection::RunNotStarted { run: id, .. }) = refused else {
    panic!("the start is refused with the reason, got {refused:?}");
  };
  fs::write(root.join(".git/info/exclude"), ".orchester/\n").expect("exclude .orchester again");
  let again = scheduler
    .run_again(IdempotencyKey::generate(), project.task, id, None)
    .await
    .expect("the person starts it again");
  wait_for(root, &mut events, again, ended).await;
  left(project, &scheduler).await
}

#[tokio::test(flavor = "multi_thread")]
async fn serialized_boundary_a_worktree_that_cannot_be_created_leaves_the_same_rows() {
  let rows = same_rows(worktree_failed, &["run_log", "invocation_start"]).await;
  let runs = rows.table("runs");
  assert_eq!(runs.len(), 2, "{runs:#?}");
  assert!(runs.iter().any(|run| run.contains("worktree")), "{runs:#?}");
}

async fn cancelled(way: Way) -> Left {
  let project = project(&Setup::new("Keep working."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_for(&project, way);
  let id = start(&scheduler, project.task).await;
  // The agent keeps working once it reported the helper it started: the last line it writes before the cancel.
  waited(root, &mut events, "the agent starts its helper", |db| {
    let helper = !reported(db, id, "helper pid: ").is_empty();
    helper.then_some(())
  })
  .await;
  scheduler.cancel_run(id).await.expect("cancelled");
  let run = wait_for(root, &mut events, id, ended).await;
  assert_eq!(run.status(), &RunStatus::Cancelled, "{way:?}");
  left(project, &scheduler).await
}

#[tokio::test(flavor = "multi_thread")]
async fn serialized_boundary_a_cancelled_run_leaves_the_same_rows() {
  same_rows(cancelled, &["run_log", "invocation_start", "activity", "invocation"]).await;
}
