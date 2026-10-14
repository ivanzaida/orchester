//! The task conversation as data: each message with who wrote it, the step it belongs to and where
//! it stands (sending now, queued, delivered mid-turn or at the end of a turn, not delivered), whom a
//! new message can go to, which agents work now, what the composer offers in the run's state, what a
//! message written now does (none once a ready run's worktree was removed), and the question a run
//! waits on in Needs you. Runs that wait for the person are in `conversation_waiting`. Sending stores a message once
//! per key; one its agent's CLI did not take is sent after the turn on request.

mod support;

use orchester_core::{
  connection::stream::InvocationOutcome,
  conversation::{AgentMessage, Arrival, Message, MessageBody, Recipient, Timing, Undelivered},
  id::{IdempotencyKey, InvocationId, ResourceId, StepExecutionId, TaskId},
  integration::{FolderListing, LeftWorktree, Leftovers, WorktreeOwner},
  resource::ModelId,
  run::{Run, WaitReason},
  store::{ActivityStore, ConversationStore},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_workspace::tasks::{
  conversation::{
    Author, Conversation, EntryState, PanelState, Queue,
    composer::{ComposerMode, Queued, Then},
    send_after_turn_in, send_in,
  },
  data::ProjectTasks,
  detail::TaskDetail,
  list::AwaitedQuestion,
  overview::OverviewTab,
};

use self::support::{Fixture, at};

fn body(text: &str) -> MessageBody {
  MessageBody::new(text).expect("a message")
}

fn read(fixture: &Fixture, task: TaskId) -> Conversation {
  let detail = TaskDetail::load_in(fixture.repo.path(), task).expect("the task is read");
  Conversation::of(&detail, false, None)
}

/// What the composer offers for a message to `recipient`.
fn mode(fixture: &Fixture, task: TaskId, recipient: Recipient) -> ComposerMode {
  let detail = TaskDetail::load_in(fixture.repo.path(), task).expect("the task is read");
  let conversation = Conversation::of(&detail, false, None);
  ComposerMode::of(&conversation, detail.run.as_ref(), recipient)
}

fn send(fixture: &mut Fixture, task: TaskId, run: Option<&Run>, text: &str, now: Timestamp) -> Message {
  let message = Message::from_person(task, run.map(Run::id), Recipient::Orchestrator, body(text), now);
  fixture
    .db
    .send_message(IdempotencyKey::generate(), &message)
    .expect("stored")
}

/// An invocation of the run's one step, started at `now`.
fn invocation(fixture: &mut Fixture, run: &Run, now: Timestamp) -> InvocationId {
  let execution = first_execution(run);
  fixture
    .db
    .start_invocation(run.id(), execution, &ModelId::new("gpt-5.5").expect("model"), now)
    .expect("invocation")
    .id
}

fn first_execution(run: &Run) -> StepExecutionId {
  run.current_attempt().expect("attempt").executions()[0].id()
}

#[test]
fn conversation_messages_read_who_wrote_them_where_they_stand_and_their_step() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  let run = fixture.started_run(task, at(20));
  let execution = first_execution(&run);
  let started = invocation(&mut fixture, &run, at(21));
  let mut delivered = send(&mut fixture, task, Some(&run), "Keep email sign-in.", at(22));
  delivered
    .deliver(run.id(), (execution, started), at(23), Arrival::Prompt)
    .expect("delivered");
  fixture
    .db
    .save_deliveries(std::slice::from_ref(&delivered))
    .expect("saved");
  send(&mut fixture, task, Some(&run), "Also cover cancel.", at(24));
  let reply = Message::from_agent(
    task,
    run.id(),
    (execution, started),
    AgentMessage::Reply,
    body("Understood."),
    at(25),
  );
  fixture.db.add_agent_message(&reply).expect("stored");

  let conversation = read(&fixture, task);
  assert_eq!(conversation.state, PanelState::Live);
  let states: Vec<(&Author, &EntryState)> = conversation
    .entries
    .iter()
    .map(|entry| (&entry.author, &entry.state))
    .collect();
  assert_eq!(
    states,
    [
      (
        &Author::You,
        &EntryState::Delivered {
          agent: "Builder".into(),
          at: at(23),
          arrival: Arrival::Prompt,
          sent_now: false,
        }
      ),
      // Sent after the turn to Builder, which works: it reads it when its turn ends.
      (&Author::You, &EntryState::Queued(Queue::AfterTurn("Builder".into()))),
      (
        &Author::Agent("Builder".into()),
        &EntryState::Reply {
          step: "Implement".into()
        }
      ),
    ]
  );
  let steps: Vec<Option<StepExecutionId>> = conversation.entries.iter().map(|entry| entry.execution).collect();
  assert_eq!(steps, [Some(execution), None, Some(execution)]);
  assert_eq!(&*conversation.entries[2].body, "Understood.");
  let recipients: Vec<Option<&str>> = conversation
    .recipients
    .iter()
    .map(|choice| choice.agent.as_deref())
    .collect();
  assert_eq!(recipients, [None, Some("Builder")]);
  assert_eq!(conversation.recipients[0].recipient, Recipient::Orchestrator);
  assert_eq!(conversation.working.len(), 1);
  assert_eq!(&*conversation.working[0].agent, "Builder");
  assert_eq!(conversation.next_agent, None);
}

#[test]
fn conversation_an_agent_works_while_its_cli_turn_runs() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  assert!(read(&fixture, task).working.is_empty());

  let mut run = fixture.started_run(task, at(20));
  assert!(read(&fixture, task).working.is_empty(), "no CLI turn has started");
  let execution = first_execution(&run);
  let mut turn = fixture
    .db
    .start_invocation(run.id(), execution, &ModelId::new("gpt-5.5").expect("model"), at(21))
    .expect("invocation");
  let working = read(&fixture, task).working;
  assert_eq!(working.len(), 1);
  assert_eq!(working[0].execution, execution);
  assert!(
    !working[0].mid_turn,
    "a profile whose connection is gone takes no message mid-turn"
  );

  let events = run.wait_step(execution, WaitReason::Approval, at(22)).expect("waits");
  fixture.record(&run, &events);
  let waiting = read(&fixture, task);
  assert_eq!(waiting.state, PanelState::Waiting);
  assert!(waiting.working.is_empty(), "the step waits for the person");

  turn.ended_at = Some(at(23));
  turn.outcome = Some(InvocationOutcome::Completed);
  fixture.db.save_invocation(&turn).expect("invocation ended");
  assert!(read(&fixture, task).working.is_empty());
}

#[test]
fn conversation_a_message_sent_now_reads_sending_then_where_it_arrived() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  let run = fixture.started_run(task, at(20));
  let execution = first_execution(&run);
  let started = invocation(&mut fixture, &run, at(21));
  let root = fixture.repo.path().to_path_buf();
  let now = |text: &str| {
    send_in(
      &root,
      IdempotencyKey::generate(),
      (task, Some(run.id()), None),
      (Recipient::Orchestrator, Timing::Now),
      text,
    )
    .expect("stored")
  };
  let (mut mid_turn, mut turn_end, mut refused) = (now("Stop editing."), now("Rename it."), now("Use the API."));
  let conversation = read(&fixture, task);
  assert!(
    conversation
      .entries
      .iter()
      .all(|entry| entry.state == EntryState::Queued(Queue::SendingNow("Builder".into())) && entry.sent_now)
  );

  mid_turn
    .deliver(run.id(), (execution, started), at(30), Arrival::MidTurn)
    .expect("delivered");
  turn_end
    .deliver(run.id(), (execution, started), at(31), Arrival::TurnEnd)
    .expect("delivered");
  refused
    .not_delivered(run.id(), execution, at(32), Undelivered::Refused)
    .expect("not delivered");
  fixture
    .db
    .save_deliveries(&[mid_turn, turn_end, refused.clone()])
    .expect("saved");
  let states: Vec<EntryState> = read(&fixture, task)
    .entries
    .into_iter()
    .map(|entry| entry.state)
    .collect();
  assert_eq!(
    states,
    [
      EntryState::Delivered {
        agent: "Builder".into(),
        at: at(30),
        arrival: Arrival::MidTurn,
        sent_now: true,
      },
      EntryState::Delivered {
        agent: "Builder".into(),
        at: at(31),
        arrival: Arrival::TurnEnd,
        sent_now: true,
      },
      EntryState::NotDelivered {
        agent: "Builder".into(),
        reason: Undelivered::Refused,
      },
    ]
  );

  // Send after this turn: queued again, after the turn.
  send_after_turn_in(&root, task, refused.id()).expect("queued again");
  let again = read(&fixture, task);
  assert_eq!(
    again.entries[2].state,
    EntryState::Queued(Queue::AfterTurn("Builder".into()))
  );
  assert!(!again.entries[2].sent_now);
  assert!(
    send_after_turn_in(&root, task, refused.id()).is_err(),
    "only a message that was not delivered"
  );
}

#[test]
fn conversation_the_composer_offers_what_the_run_state_allows() {
  let mut fixture = Fixture::reviewed();
  let task = fixture.task("Add login", at(10));
  assert_eq!(mode(&fixture, task, Recipient::Orchestrator), ComposerMode::NotStarted);

  let mut run = fixture.started_run(task, at(20));
  assert_eq!(
    mode(&fixture, task, Recipient::Orchestrator),
    ComposerMode::Queue(Queued::Checkpoint),
    "no agent at work yet"
  );
  invocation(&mut fixture, &run, at(21));
  let recipients = read(&fixture, task).recipients;
  let (builder, reviewer) = (recipients[1].recipient, recipients[2].recipient);
  for recipient in [Recipient::Orchestrator, builder] {
    let ComposerMode::Steer(working) = mode(&fixture, task, recipient) else {
      panic!("a message for Builder, at work, can be sent now or after its turn");
    };
    assert_eq!(&*working.agent, "Builder");
  }
  assert_eq!(
    mode(&fixture, task, reviewer),
    ComposerMode::Queue(Queued::NextTurn(Some("Reviewer".into())))
  );
  // A recipient the run's pipeline does not offer is not named with an empty name.
  assert_eq!(
    mode(&fixture, task, Recipient::Agent(ResourceId::generate())),
    ComposerMode::Queue(Queued::NextTurn(None))
  );

  let execution = first_execution(&run);
  let events = run.wait_step(execution, WaitReason::Message, at(22)).expect("waits");
  fixture.record(&run, &events);
  let started = fixture.db.invocations(run.id()).expect("invocations")[0].id;
  let question = Message::from_agent(
    task,
    run.id(),
    (execution, started),
    AgentMessage::Question,
    body("Keep email sign-in?"),
    at(23),
  );
  fixture.db.add_agent_message(&question).expect("stored");
  let ComposerMode::Answer { agent, asked_at, .. } = mode(&fixture, task, Recipient::Orchestrator) else {
    panic!("the composer answers the question");
  };
  assert_eq!((&*agent, asked_at), ("Builder", at(23)));

  let events = run.pause(at(24)).expect("paused");
  fixture.record(&run, &events);
  let paused = mode(&fixture, task, Recipient::Orchestrator);
  assert_eq!(paused, ComposerMode::Resume { interrupted: false });
  // Resume and send, or the message waits until the run resumes.
  assert_eq!(
    (paused.then(), paused.then_later()),
    (Some(Then::Resume), Some(Then::Deliver))
  );
}

#[test]
fn conversation_a_question_waits_for_the_answer_and_needs_you() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  let mut run = fixture.started_run(task, at(20));
  let execution = first_execution(&run);
  let started = invocation(&mut fixture, &run, at(21));
  let events = run.wait_step(execution, WaitReason::Message, at(22)).expect("waits");
  fixture.record(&run, &events);
  let question = Message::from_agent(
    task,
    run.id(),
    (execution, started),
    AgentMessage::Question,
    body("Keep email sign-in?"),
    at(22),
  );
  fixture.db.add_agent_message(&question).expect("stored");

  let conversation = read(&fixture, task);
  assert_eq!(conversation.state, PanelState::Waiting);
  let open = conversation.open_question().expect("the question waits");
  assert_eq!(&*open.body, "Keep email sign-in?");
  assert_eq!(open.execution, Some(execution));
  assert_eq!(conversation.next_agent.as_deref(), Some("Builder"));

  let tasks = ProjectTasks::load(fixture.repo.path(), &Programs::processes()).expect("load the project's tasks");
  assert_eq!(OverviewTab::NeedsYou.count(&tasks.tasks, &tasks.phases), 1);
  let rows = OverviewTab::NeedsYou.rows(&tasks.tasks, &tasks.pipelines, &tasks.phases, 3);
  assert_eq!(
    rows[0].question,
    Some(AwaitedQuestion {
      agent: "Builder".into(),
      text: "Keep email sign-in?".into(),
    })
  );
  assert_eq!(rows[0].plan, None);

  // Answered, the step works again and the question is closed.
  let events = run.resume_step(execution, at(30)).expect("resumed");
  fixture.record(&run, &events);
  let conversation = read(&fixture, task);
  assert_eq!(conversation.state, PanelState::Live);
  assert_eq!(conversation.open_question(), None);
  assert_eq!(conversation.entries[0].state, EntryState::Question { open: false });
}

#[test]
fn conversation_what_a_message_written_now_does_follows_the_run() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  send(&mut fixture, task, None, "Keep email sign-in.", at(11));
  let before = read(&fixture, task);
  assert_eq!(before.state, PanelState::NotStarted);
  assert_eq!(before.entries[0].state, EntryState::Queued(Queue::RunStarts));
  assert_eq!(before.recipients.len(), 1, "only Orchestrator before a run");

  let mut run = fixture.started_run(task, at(20));
  fixture.succeed(&mut run, at(30));
  let ready = read(&fixture, task);
  assert_eq!(ready.state, PanelState::Ready);
  assert!(ready.working.is_empty());
  assert_eq!(ready.next_agent.as_deref(), Some("Builder"));
  assert!(ready.state.takes_messages());
  assert_eq!(mode(&fixture, task, Recipient::Orchestrator), ComposerMode::FollowUp);

  let other = fixture.task("Add logout", at(40));
  let mut cancelled = fixture.started_run(other, at(41));
  send(&mut fixture, other, Some(&cancelled), "Stop.", at(42));
  let events = cancelled.cancel(at(43)).expect("cancelled");
  fixture.record(&cancelled, &events);
  let ended = read(&fixture, other);
  assert_eq!(ended.state, PanelState::Ended);
  assert!(!ended.state.takes_messages());
  assert_eq!(ended.entries[0].state, EntryState::Queued(Queue::RunEnded));
  // No agent of the run reads it: the task runs again with the message, for its next run.
  let again = mode(&fixture, other, Recipient::Orchestrator);
  assert_eq!(again, ComposerMode::RunAgain);
  assert_eq!(again.then(), Some(Then::RunAgain));
  assert!(Then::RunAgain.for_next_run());
  send(&mut fixture, other, None, "Start over with the new API.", at(44));
  assert_eq!(
    read(&fixture, other).entries[1].state,
    EntryState::Queued(Queue::NextRun)
  );
}

#[test]
fn conversation_live_direct_message_to_later_agent_waits_for_its_turn() {
  let mut fixture = Fixture::reviewed();
  let task = fixture.task("Add login", at(10));
  let run = fixture.started_run(task, at(20));
  invocation(&mut fixture, &run, at(21));
  let reviewer = read(&fixture, task).recipients[2].recipient;
  send_in(
    fixture.repo.path(),
    IdempotencyKey::generate(),
    (task, Some(run.id()), None),
    (reviewer, Timing::AfterTurn),
    "Review this after implementation.",
  )
  .expect("later agent message stored");

  let conversation = read(&fixture, task);
  assert_eq!(conversation.state, PanelState::Live);
  assert_eq!(conversation.entries[0].author, Author::YouTo("Reviewer".into()));
  assert_eq!(
    conversation.entries[0].state,
    EntryState::Queued(Queue::AgentNextTurn("Reviewer".into()))
  );
}

#[test]
fn conversation_a_ready_run_whose_worktree_was_removed_takes_no_messages() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  let mut run = fixture.started_run(task, at(20));
  fixture.succeed(&mut run, at(30));
  send(&mut fixture, task, Some(&run), "Add a second line.", at(31));
  let detail = TaskDetail::load_in(fixture.repo.path(), task).expect("the task is read");
  let run_worktree = |missing| LeftWorktree {
    path: "/repo/.orchester/worktrees/run".to_owned(),
    owner: WorktreeOwner::Run,
    branch: None,
    uncommitted: Vec::new(),
    missing,
    forgotten: false,
    listing: FolderListing::Whole,
  };
  let kept = Leftovers {
    worktrees: vec![run_worktree(false)],
    branches: Vec::new(),
  };
  let ready = Conversation::of(&detail, false, Some(&kept));
  assert_eq!(ready.state, PanelState::Ready);
  assert_eq!(ready.entries[0].state, EntryState::Queued(Queue::NextCheckpoint));

  let deleted_by_hand = Leftovers {
    worktrees: vec![run_worktree(true)],
    branches: Vec::new(),
  };
  for leftovers in [Leftovers::default(), deleted_by_hand] {
    let removed = Conversation::of(&detail, false, Some(&leftovers));
    assert_eq!(removed.state, PanelState::WorktreeRemoved);
    assert!(!removed.state.takes_messages());
    assert_eq!(removed.entries[0].state, EntryState::Queued(Queue::WorktreeRemoved));
    assert_eq!(
      ComposerMode::of(&removed, detail.run.as_ref(), Recipient::Orchestrator),
      ComposerMode::Closed
    );
  }
}

#[test]
fn conversation_sending_stores_a_message_once_per_key() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  let run = fixture.started_run(task, at(20));
  let key = IdempotencyKey::generate();
  let root = fixture.repo.path().to_path_buf();
  let to = (Recipient::Orchestrator, Timing::AfterTurn);
  let first = send_in(&root, key, (task, Some(run.id()), None), to, "  Hi.  ").expect("stored");
  assert_eq!(first.body().as_str(), "Hi.");
  let again = send_in(&root, key, (task, Some(run.id()), None), to, "Hi.").expect("the first");
  assert_eq!(again, first);
  assert!(send_in(&root, IdempotencyKey::generate(), (task, None, None), to, "  ").is_err());
  assert_eq!(read(&fixture, task).entries.len(), 1);
}

#[test]
fn conversation_failed_run_accepts_explicit_recovery_and_disables_removed_worktree() {
  let mut fixture = Fixture::reviewed();
  let task = fixture.task("Add login", at(10));
  let mut run = fixture.started_run(task, at(20));
  let execution = first_execution(&run);
  let events = run
    .fail_step(execution, orchester_core::run::StepFailure::ReportedFailed, at(30))
    .expect("failure");
  fixture.record(&run, &events);
  let conversation = read(&fixture, task);
  assert_eq!(conversation.state, PanelState::Failed);
  assert!(conversation.state.takes_messages());
  assert_eq!(conversation.next_agent.as_deref(), Some("Builder"));
  assert_eq!(conversation.recipients.len(), 2, "unexecuted Reviewer is unavailable");
  assert_eq!(conversation.recovery_after, Some(run.last_sequence()));
  assert_eq!(mode(&fixture, task, Recipient::Orchestrator), ComposerMode::Continue);
  let message = send_in(
    fixture.repo.path(),
    IdempotencyKey::generate(),
    (task, Some(run.id()), conversation.recovery_after),
    (Recipient::Orchestrator, Timing::AfterTurn),
    "Continue with my answer.",
  )
  .expect("sent");
  assert_eq!(message.recovery_after(), Some(run.last_sequence()));
  let saved = fixture.db.messages(task).expect("messages");
  assert_eq!(saved[0], message, "recovery intent survives persistence");
  let detail = TaskDetail::load_in(fixture.repo.path(), task).expect("detail");
  let removed = Conversation::of(&detail, false, Some(&Leftovers::default()));
  assert_eq!(removed.state, PanelState::FailedWorktreeRemoved);
  assert!(!removed.state.takes_messages());
  assert_eq!(removed.recovery_after, None);
  assert_eq!(removed.entries[0].state, EntryState::Queued(Queue::WorktreeRemoved));
}
