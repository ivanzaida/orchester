//! Agent feedback on a pull request: a thread starts on lines or on the whole pull request for the agent of the
//! linked task, and goes to it as a message of the task's conversation once the run's state lets it; a second fix
//! request waits for the first, a question does not; a follow-up waits for an agent slot; what Orchester holds can
//! be withdrawn; where a thread stands follows the conversation (in it, taken, answered, a fix ready to verify);
//! a thread's lines follow new commits or turn outdated; resolving closes it in Orchester only.

use std::{num::NonZeroU32, path::Path};

use orchester_core::{
  conversation::{Arrival, Message, Recipient, Sender},
  diff::Diff,
  id::{IdempotencyKey, ResourceId, StepExecutionId},
  pull_request::{
    PullRequestState,
    feedback::{DiffSide, FeedbackAnchor, FeedbackKind, LineAnchor, LineSpan, Located, locate},
  },
  resource::ModelId,
  revision::Revision,
  run::Run,
  store::{ActivityStore, ConversationStore, PullRequestStore},
};
use orchester_exec::programs::Programs;
use orchester_workspace::pull_requests::feedback::{
  FeedbackBoard, FeedbackError, Gate, Hold, Inputs, NewThread, Slots, ThreadState, dispatch_in, load_board_in,
  reply_in, resolve_in, start_in, withdraw_in,
};

use super::{
  link, pull_request, repo,
  support::{Fixture, at},
};

/// The head of pull request `number` in the fixture's list.
fn head(number: u64) -> Revision {
  Revision::new(&format!("{number:0>40}")).expect("revision")
}

fn span(start: u32, end: u32) -> LineSpan {
  LineSpan::new(
    NonZeroU32::new(start).expect("a line"),
    NonZeroU32::new(end).expect("a line"),
  )
  .expect("a span")
}

fn lines(path: &str, (start, end): (u32, u32), code: &[&str]) -> FeedbackAnchor {
  FeedbackAnchor::Lines(
    LineAnchor::new(
      path,
      DiffSide::New,
      span(start, end),
      code.iter().map(|line| (*line).to_owned()).collect(),
    )
    .expect("an anchor"),
  )
}

/// KON-1 runs on the branch of #142; KON-2's run, the branch of #137, succeeded.
struct Project {
  fixture: Fixture,
  working: Run,
  builder: ResourceId,
}

fn project() -> Project {
  let mut fixture = Fixture::keyed("KON");
  let working_task = fixture.task("Add retry to webhook sender", at(10));
  let working = fixture.started_run(working_task, at(20));
  let done = fixture.task("Typed errors per module", at(11));
  let mut finished = fixture.started_run(done, at(21));
  fixture.succeed(&mut finished, at(30));
  link(&mut fixture);
  let stored = [
    pull_request(142, "orchester/orc-run-1", PullRequestState::Open, None, 900),
    pull_request(137, "orchester/orc-run-2", PullRequestState::Open, None, 800),
    pull_request(139, "deps/tokio", PullRequestState::Draft, None, 700),
  ];
  fixture
    .db
    .replace_pull_requests(&repo(), &stored, at(1_000))
    .expect("the pull requests are stored");
  let builder = builder_of(&working);
  Project {
    fixture,
    working,
    builder,
  }
}

fn builder_of(run: &Run) -> ResourceId {
  match run.snapshot().steps()[0].executor() {
    orchester_core::run::StepExecutor::Agent { profile, .. } => profile.resource,
    orchester_core::run::StepExecutor::Tool { .. } => panic!("Implement is an agent step"),
  }
}

fn new_thread(project: &Project, number: u64, kind: FeedbackKind, anchor: FeedbackAnchor, text: &str) -> NewThread {
  NewThread {
    number,
    anchor,
    kind,
    text: text.to_owned(),
    revision: head(number),
    recipient: project.builder,
  }
}

fn start(project: &Project, new: &NewThread) -> orchester_core::id::FeedbackId {
  start_in(root(project), IdempotencyKey::generate(), new)
    .expect("the thread is kept")
    .id()
}

fn root(project: &Project) -> &Path {
  project.fixture.repo.path()
}

fn board(project: &Project, number: u64, slots: Option<Slots>) -> FeedbackBoard {
  let programs = Programs::processes();
  let inputs = Inputs {
    root: root(project),
    programs: &programs,
    head: None,
    slots,
    lost: false,
  };
  load_board_in(&inputs, number).expect("the board is read")
}

fn messages(project: &Project, run: &Run) -> Vec<Message> {
  project.fixture.db.messages(run.task()).expect("the messages are read")
}

fn first_execution(run: &Run) -> StepExecutionId {
  run.current_attempt().expect("attempt").executions()[0].id()
}

#[test]
fn feedback_on_lines_goes_to_the_builder_with_its_code() {
  let project = project();
  let fix = new_thread(
    &project,
    142,
    FeedbackKind::Fix,
    lines(
      "src/webhooks/sender.rs",
      (43, 44),
      &["attempt += 1;", "let started = Instant::now();"],
    ),
    "Check for cancellation before each attempt.",
  );
  start(&project, &fix);
  let sent = dispatch_in(root(&project), None, false).expect("dispatched");
  assert_eq!(sent.len(), 1, "KON-1's run is told");
  let stored = messages(&project, &project.working);
  let [message] = stored.as_slice() else {
    panic!("one message is in KON-1's conversation: {stored:?}");
  };
  assert!(matches!(
    message.sender(),
    Sender::Person { recipient: Recipient::Agent(to), .. } if *to == project.builder
  ));
  let text = message.body().as_str();
  assert!(text.starts_with("Fix request for pull request #142"), "{text}");
  assert!(
    text.contains("src/webhooks/sender.rs, new lines 43-44 at 0000000"),
    "{text}"
  );
  assert!(text.contains("attempt += 1;\nlet started = Instant::now();"), "{text}");
  assert!(text.ends_with("Check for cancellation before each attempt."), "{text}");
  let board = board(&project, 142, None);
  assert!(
    matches!(board.threads[0].state, ThreadState::Delivered(_)),
    "{:?}",
    board.threads[0].state
  );
  assert_eq!(
    dispatch_in(root(&project), None, false).expect("dispatched"),
    Vec::new(),
    "sent once"
  );
}

#[test]
fn feedback_fix_requests_change_the_branch_one_at_a_time_and_questions_do_not_wait() {
  let project = project();
  let whole = FeedbackAnchor::PullRequest;
  let first = start(
    &project,
    &new_thread(&project, 142, FeedbackKind::Fix, whole.clone(), "First fix."),
  );
  let second = start(
    &project,
    &new_thread(&project, 142, FeedbackKind::Fix, whole.clone(), "Second fix."),
  );
  let question = start(
    &project,
    &new_thread(&project, 142, FeedbackKind::Question, whole, "Why?"),
  );
  dispatch_in(root(&project), None, false).expect("dispatched");
  let board = board(&project, 142, None);
  let state = |id| board.thread(id).expect("the thread").state.clone();
  assert!(matches!(state(first), ThreadState::Delivered(_)));
  assert_eq!(state(second), ThreadState::Queued(Hold::AfterFix(first)));
  assert!(matches!(state(question), ThreadState::Delivered(_)));
  assert_eq!(messages(&project, &project.working).len(), 2);
}

#[test]
fn feedback_held_requests_are_withdrawn_and_sent_ones_are_not() {
  let project = project();
  let whole = FeedbackAnchor::PullRequest;
  let first = start(
    &project,
    &new_thread(&project, 142, FeedbackKind::Fix, whole.clone(), "First fix."),
  );
  let second = start(
    &project,
    &new_thread(&project, 142, FeedbackKind::Fix, whole, "Second fix."),
  );
  dispatch_in(root(&project), None, false).expect("dispatched");
  let board = board(&project, 142, None);
  let held = board
    .thread(second)
    .and_then(|thread| thread.held)
    .expect("the second is held");
  assert!(withdraw_in(root(&project), (142, second), held).expect("withdrawn"));
  let sent = board.thread(first).expect("the first").current().id;
  assert!(
    !withdraw_in(root(&project), (142, first), sent).expect("read"),
    "a sent request stays"
  );
  let board = self::board(&project, 142, None);
  assert!(matches!(
    board.thread(second).expect("the second").state,
    ThreadState::Withdrawn { .. }
  ));
  assert_eq!(board.open().count(), 1);
}

#[test]
fn feedback_states_follow_the_conversation_up_to_a_fix_ready_to_verify() {
  let mut project = project();
  let anchor = FeedbackAnchor::PullRequest;
  let fix = start(
    &project,
    &new_thread(&project, 142, FeedbackKind::Fix, anchor, "Share the body as Bytes."),
  );
  dispatch_in(root(&project), None, false).expect("dispatched");
  let run = project.working.clone();
  let execution = first_execution(&run);
  let invocation = project
    .fixture
    .db
    .start_invocation(run.id(), execution, &ModelId::new("gpt-5.5").expect("model"), at(40))
    .expect("invocation")
    .id;
  let mut message = messages(&project, &run).remove(0);
  message
    .deliver(run.id(), (execution, invocation), at(41), Arrival::Prompt)
    .expect("delivered");
  project
    .fixture
    .db
    .save_deliveries(std::slice::from_ref(&message))
    .expect("saved");
  assert_eq!(
    board(&project, 142, None).thread(fix).expect("thread").state,
    ThreadState::Working { since: at(41) }
  );
  let mut moved = run;
  let commit = Revision::new(&"b".repeat(40)).expect("revision");
  let events = moved
    .record_checkpoint(execution, commit.clone(), at(50))
    .expect("checkpoint");
  project.fixture.record(&moved, &events);
  message.acknowledge(at(55)).expect("acknowledged");
  project
    .fixture
    .db
    .save_deliveries(std::slice::from_ref(&message))
    .expect("saved");
  let board = board(&project, 142, None);
  let thread = board.thread(fix).expect("thread");
  assert_eq!(
    thread.state,
    ThreadState::ReadyToVerify {
      at: at(55),
      commit: commit.clone()
    }
  );
  assert_eq!(thread.commit.as_ref().map(|commit| &commit.revision), Some(&commit));
  assert!(resolve_in(root(&project), (142, fix), true).expect("resolved"));
  assert!(matches!(
    self::board(&project, 142, None).thread(fix).expect("thread").state,
    ThreadState::Resolved { .. }
  ));
  assert!(resolve_in(root(&project), (142, fix), false).expect("reopened"));
}

#[test]
fn feedback_a_follow_up_waits_for_an_agent_slot() {
  let project = project();
  let question = start(
    &project,
    &new_thread(
      &project,
      137,
      FeedbackKind::Question,
      FeedbackAnchor::PullRequest,
      "Why?",
    ),
  );
  let board = board(&project, 137, None);
  let task = board.task.as_ref().expect("#137 is KON-2's");
  assert!(matches!(task.gate, Gate::FollowUp { .. }), "{:?}", task.gate);
  let full = Some(Slots { working: 4, slots: 4 });
  assert_eq!(
    dispatch_in(root(&project), full, false).expect("dispatched"),
    Vec::new()
  );
  assert_eq!(
    self::board(&project, 137, full).thread(question).expect("thread").state,
    ThreadState::Queued(Hold::NoSlot { working: 4, slots: 4 })
  );
  let free = Some(Slots { working: 1, slots: 4 });
  assert_eq!(dispatch_in(root(&project), free, false).expect("dispatched").len(), 1);
}

#[test]
fn feedback_needs_a_linked_task_and_the_head_the_person_saw() {
  let project = project();
  let unlinked = new_thread(
    &project,
    139,
    FeedbackKind::Question,
    FeedbackAnchor::PullRequest,
    "Why?",
  );
  assert!(matches!(
    start_in(root(&project), IdempotencyKey::generate(), &unlinked),
    Err(FeedbackError::NoTask)
  ));
  assert_eq!(board(&project, 139, None).task, None);
  let stale = NewThread {
    revision: Revision::new(&"c".repeat(40)).expect("revision"),
    ..new_thread(&project, 142, FeedbackKind::Fix, FeedbackAnchor::PullRequest, "Fix it.")
  };
  assert!(matches!(
    start_in(root(&project), IdempotencyKey::generate(), &stale),
    Err(FeedbackError::HeadMoved(moved)) if moved == head(142)
  ));
}

#[test]
fn feedback_replies_reopen_a_resolved_thread_and_ask_again() {
  let project = project();
  let anchor = FeedbackAnchor::PullRequest;
  let thread = start(
    &project,
    &new_thread(&project, 142, FeedbackKind::Question, anchor, "Why?"),
  );
  resolve_in(root(&project), (142, thread), true).expect("resolved");
  let replied = reply_in(
    root(&project),
    IdempotencyKey::generate(),
    (142, thread),
    (FeedbackKind::Fix, "Then fix it.", &head(142)),
  )
  .expect("replied");
  assert_eq!(replied.resolved_at(), None);
  assert_eq!(replied.kind(), FeedbackKind::Fix);
  assert_eq!(replied.posts().len(), 2);
}

#[test]
fn feedback_lines_follow_the_same_code_or_turn_outdated() {
  let diff = Diff::parse(
    "diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1,3 +1,5 @@\n one\n+guard\n+check\n two\n \
     three\n",
  )
  .expect("a diff");
  let file = diff.files().first();
  let anchor = LineAnchor::new(
    "src/a.rs",
    DiffSide::New,
    span(2, 3),
    vec!["two".into(), "three".into()],
  )
  .expect("an anchor");
  assert_eq!(locate(&anchor, file), Located::At(span(4, 5)));
  let gone = LineAnchor::new("src/a.rs", DiffSide::New, span(2, 2), vec!["removed".into()]).expect("an anchor");
  assert_eq!(locate(&gone, file), Located::Outdated);
  assert_eq!(locate(&anchor, None), Located::Outdated);
}
