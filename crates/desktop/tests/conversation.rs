//! The task conversation as data: each message with who wrote it and where it stands, whom a new
//! message can go to, who takes part, what the run's state means for a message written now (none
//! once a ready run's worktree was removed), and the question a run waits on in Needs you. Sending
//! stores a message once per key.

mod support;

use orchester_core::{
  conversation::{AgentMessage, Message, MessageBody, Recipient},
  id::{IdempotencyKey, InvocationId, TaskId},
  integration::{LeftWorktree, Leftovers, WorktreeOwner},
  resource::ModelId,
  run::{Run, WaitReason},
  store::{ActivityStore, ConversationStore},
  time::Timestamp,
};
use orchester_desktop::tasks::{
  conversation::{Author, Conversation, EntryState, PanelState, Queue, send_in},
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

fn send(fixture: &mut Fixture, task: TaskId, run: Option<&Run>, text: &str, now: Timestamp) -> Message {
  let message = Message::from_person(task, run.map(Run::id), Recipient::Orchestrator, body(text), now);
  fixture
    .db
    .send_message(IdempotencyKey::generate(), &message)
    .expect("stored")
}

/// An invocation of the run's one step, started at `now`.
fn invocation(fixture: &mut Fixture, run: &Run, now: Timestamp) -> InvocationId {
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  fixture
    .db
    .start_invocation(run.id(), execution, &ModelId::new("gpt-5.5").expect("model"), now)
    .expect("invocation")
    .id
}

#[test]
fn conversation_messages_read_who_wrote_them_and_where_they_stand() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  let run = fixture.started_run(task, at(20));
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let started = invocation(&mut fixture, &run, at(21));
  let mut delivered = send(&mut fixture, task, Some(&run), "Keep email sign-in.", at(22));
  delivered
    .deliver(run.id(), (execution, started), at(23))
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
          agent: "Builder".into()
        }
      ),
      (&Author::You, &EntryState::Queued(Queue::NextCheckpoint)),
      (
        &Author::Agent("Builder".into()),
        &EntryState::Reply {
          step: "Implement".into()
        }
      ),
    ]
  );
  assert_eq!(&*conversation.entries[2].body, "Understood.");
  let recipients: Vec<Option<&str>> = conversation
    .recipients
    .iter()
    .map(|choice| choice.agent.as_deref())
    .collect();
  assert_eq!(recipients, [None, Some("Builder")]);
  assert_eq!(conversation.recipients[0].recipient, Recipient::Orchestrator);
  assert_eq!(conversation.participants.working, ["Builder".into()]);
  assert!(conversation.participants.available.is_empty());
  assert_eq!(conversation.next_agent, None);
}

#[test]
fn conversation_a_question_waits_for_the_answer_and_needs_you() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  let mut run = fixture.started_run(task, at(20));
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
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
  assert_eq!(conversation.next_agent.as_deref(), Some("Builder"));
  assert_eq!(conversation.participants.asking, ["Builder".into()]);

  let tasks = ProjectTasks::load(fixture.repo.path()).expect("load the project's tasks");
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
  assert_eq!(ready.next_agent.as_deref(), Some("Builder"));
  assert!(ready.state.takes_messages());

  let other = fixture.task("Add logout", at(40));
  let mut cancelled = fixture.started_run(other, at(41));
  send(&mut fixture, other, Some(&cancelled), "Stop.", at(42));
  let events = cancelled.cancel(at(43)).expect("cancelled");
  fixture.record(&cancelled, &events);
  let ended = read(&fixture, other);
  assert_eq!(ended.state, PanelState::Ended);
  assert!(!ended.state.takes_messages());
  assert_eq!(ended.entries[0].state, EntryState::Queued(Queue::RunEnded));
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
  }
}

#[test]
fn conversation_sending_stores_a_message_once_per_key() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  let run = fixture.started_run(task, at(20));
  let key = IdempotencyKey::generate();
  let root = fixture.repo.path().to_path_buf();
  let first = send_in(&root, key, (task, Some(run.id())), Recipient::Orchestrator, "  Hi.  ").expect("stored");
  assert_eq!(first.body().as_str(), "Hi.");
  let again = send_in(&root, key, (task, Some(run.id())), Recipient::Orchestrator, "Hi.").expect("the first");
  assert_eq!(again, first);
  assert!(
    send_in(
      &root,
      IdempotencyKey::generate(),
      (task, None),
      Recipient::Orchestrator,
      "  "
    )
    .is_err()
  );
  assert_eq!(read(&fixture, task).entries.len(), 1);
}
