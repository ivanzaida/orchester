//! A person's review question, answered end to end against `orchester-fake-claude`: the run's implementing step's
//! agent answers in a read-only turn in the run's worktree, its step's session resumed as a fork, its words kept in the
//! thread; the run keeps waiting for review and no code changes; *Stop answering* ends the turn and keeps what it
//! wrote. Run with `cargo test -p orchester-runner --features test-agent --test review_answer`.

mod support;

use std::{fs, num::NonZeroU32, path::Path};

use orchester_core::{
  connection::AgentProvider,
  conversation::MessageBody,
  id::{ReviewMessageId, RunId},
  pull_request::feedback::{DiffSide, LineAnchor, LineSpan},
  resource::StepKey,
  review::{QuestionState, ReviewThread, Said, ThreadKind},
  run::{Run, RunStatus},
  store::{ActivityStore, RunStore},
  time::Timestamp,
};
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{Setup, project, scheduler, wait_for};

/// A run of the project at `root` that succeeded, waiting for review.
async fn succeeded(
  root: &Path,
  scheduler: &Scheduler,
  events: &mut mpsc::UnboundedReceiver<orchester_ipc::Event>,
  task: orchester_core::id::TaskId,
) -> Run {
  let id = scheduler
    .start_run(orchester_core::id::IdempotencyKey::generate(), task)
    .await
    .expect("the run starts");
  let run = wait_for(root, events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  run
}

/// Asks `text` about the first line of `README.md` at `run`'s head; the question.
fn ask(root: &Path, run: &Run, text: &str) -> ReviewMessageId {
  let line = NonZeroU32::new(1).expect("a line");
  let anchor =
    LineAnchor::new("README.md", DiffSide::New, LineSpan::line(line), vec!["hi".to_owned()]).expect("anchor");
  let head = run.last_checkpoint().expect("a checkpoint").revision().clone();
  let body = MessageBody::new(text).expect("words");
  let thread = ReviewThread::start(run.id(), (ThreadKind::Question, anchor, head), &body, Timestamp::now());
  ProjectDb::open(root)
    .expect("open")
    .add_review_thread(&thread)
    .expect("kept");
  thread.latest_ask().id()
}

/// The question's thread once `done` says so of the question's state and the words written after it.
async fn question(
  root: &Path,
  events: &mut mpsc::UnboundedReceiver<orchester_ipc::Event>,
  (run, question): (RunId, ReviewMessageId),
  done: impl Fn(&QuestionState, &str) -> bool,
) -> ReviewThread {
  let read = || {
    ProjectDb::open(root)
      .expect("open")
      .review_thread_of(question)
      .expect("read")
      .expect("kept")
  };
  let reached = |thread: &ReviewThread| {
    let Some(Said::Question(state)) = thread.message(question).map(|message| message.said().clone()) else {
      panic!("a question");
    };
    done(&state, &written(thread))
  };
  wait_for(root, events, run, |_| reached(&read())).await;
  read()
}

/// The words the agent wrote last in `thread`.
fn written(thread: &ReviewThread) -> String {
  thread
    .messages()
    .last()
    .filter(|words| !words.said().is_person())
    .map(|words| words.body().to_owned())
    .unwrap_or_default()
}

#[tokio::test(flavor = "multi_thread")]
async fn review_answer_the_implementing_steps_agent_answers_read_only_and_the_run_keeps_waiting() {
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    ..Setup::new("Append one line.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let run = succeeded(root, &scheduler, &mut events, project.task).await;
  let asked = ask(root, &run, "Why one line?");
  scheduler
    .answer_review_question(run.id(), StepKey::new("implement").expect("key"), asked)
    .await
    .expect("the turn starts");
  let taken = ProjectDb::open(root)
    .expect("open")
    .review_thread_of(asked)
    .expect("read")
    .expect("kept");
  assert!(
    !matches!(
      taken.message(asked).map(|question| question.said()),
      Some(Said::Question(QuestionState::Asked))
    ),
    "a question the runner took is never asked any more: the desktop recovers one still asked"
  );
  let thread = question(root, &mut events, (run.id(), asked), |state, _| {
    *state == QuestionState::Answered
  })
  .await;
  let words = thread.messages().last().expect("Implement's words");
  assert!(
    matches!(words.said(), Said::Agent(agent) if !agent.writing),
    "{words:?}"
  );
  assert!(
    words.body().starts_with("Noted: ") && words.body().contains("Why one line?"),
    "{}",
    words.body()
  );
  assert!(
    words.body().contains("read-only"),
    "the prompt says the turn changes nothing: {}",
    words.body()
  );

  unchanged(root, &run);
  let started = started(root);
  assert!(started.contains(" --tools Read,Glob,Grep "), "{started}");
  assert!(
    started.contains(" --fork-session"),
    "the step's session is resumed as a fork: {started}"
  );
  assert!(!started.contains("--mcp-config"), "no MCP server: {started}");
  assert!(!started.contains("--dangerously-skip-permissions"), "{started}");
}

#[tokio::test(flavor = "multi_thread")]
async fn review_answer_on_codex_resumes_the_steps_thread_without_any_mcp_server() {
  let project = project(&Setup::new("Append one line."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let run = succeeded(root, &scheduler, &mut events, project.task).await;
  let asked = ask(root, &run, "Why one line?");
  scheduler
    .answer_review_question(run.id(), StepKey::new("implement").expect("key"), asked)
    .await
    .expect("the turn starts");
  let thread = question(root, &mut events, (run.id(), asked), |state, _| {
    *state == QuestionState::Answered
  })
  .await;
  assert!(written(&thread).starts_with("Noted: "), "{}", written(&thread));
  unchanged(root, &run);
  let session = ProjectDb::open(root)
    .expect("open")
    .invocations(run.id())
    .expect("invocations")
    .into_iter()
    .rev()
    .find_map(|invocation| invocation.session)
    .expect("the step's thread");
  let started = started(root);
  assert!(started.contains(&format!("thread: {session}")), "{started}");
  assert!(!started.contains("mcp-url"), "no MCP server: {started}");
}

/// The run keeps waiting for review, at the same checkpoint: no code changed.
fn unchanged(root: &Path, run: &Run) {
  let after = ProjectDb::open(root)
    .expect("open")
    .run(run.id())
    .expect("read")
    .expect("run");
  assert_eq!(
    after.status(),
    &RunStatus::Succeeded,
    "the run keeps waiting for review"
  );
  assert_eq!(after.last_checkpoint(), run.last_checkpoint(), "no code changed");
}

/// How the fake CLI was started for the answer, as it logged it in the run's worktree.
fn started(root: &Path) -> String {
  let log = fs::read_dir(root.join(".orchester"))
    .expect("folders")
    .flatten()
    .flat_map(|entry| fs::read_dir(entry.path()).into_iter().flatten().flatten())
    .map(|entry| entry.path().join(".orchester").join("fake-chat.log"))
    .find(|log| log.is_file())
    .expect("the fake logged its start in the run's worktree");
  fs::read_to_string(log).expect("log")
}

#[tokio::test(flavor = "multi_thread")]
async fn review_answer_stop_answering_keeps_what_was_written() {
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    ..Setup::new("Append one line.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let run = succeeded(root, &scheduler, &mut events, project.task).await;
  let asked = ask(root, &run, "Keep thinking about it.");
  scheduler
    .answer_review_question(run.id(), StepKey::new("implement").expect("key"), asked)
    .await
    .expect("the turn starts");
  let thread = question(root, &mut events, (run.id(), asked), |state, words| {
    *state == QuestionState::Answering && !words.is_empty()
  })
  .await;
  scheduler.stop_review_answer(run.id(), thread.id()).expect("stopped");
  let thread = question(root, &mut events, (run.id(), asked), |state, _| {
    *state == QuestionState::Stopped
  })
  .await;
  assert_eq!(written(&thread), "Let me look at the queue.");
}
