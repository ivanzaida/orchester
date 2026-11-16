//! Events that wait in one conversation are taken in one turn (`docs/plan/orchestrator-autonomy/
//! OA-07-events-together.md`, `orchestrator::turns`): the turn the oldest event starts takes the next ones of its
//! conversation that are due, five events at most, and is told all of them; each thinks and ends as that turn does,
//! so a turn that failed leaves each to be tried again, and one that was never taken gives each back; a success whose
//! turn merges its run is taken only while no merge of another conversation waits before it.
//! `cargo test -p orchester-workspace --test orchestrator_together together`.

mod support;

use std::collections::HashSet;

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
  },
  conversation::{AgentMessage, Message, MessageBody},
  id::{ChatId, ChatMessageId, ConnectionId, TaskId},
  name::Name,
  orchestrator::{
    AgentWait, Chat, EntryKind, OrchestratorSettings, Subscription, TurnFailure, TurnState, WaitTurn, WatchTarget,
  },
  resource::ModelId,
  run::{Run, WaitReason},
  store::{ActivityStore, ConnectionStore, ConversationStore},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::orchestrator::{
  chat::ChatView,
  waits::{Followed, Switches, check_merges_in, follow_at},
};
use support::{Fixture, at};

/// A checked, signed-in Claude Code connection, published as what the Orchestrator runs on.
fn ready(fixture: &mut Fixture) {
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Claude · Kontur").expect("name"),
    AgentProvider::ClaudeCodeCli,
    fixture.repo.path().join("claude.exe"),
    Timestamp::now(),
  )
  .expect("absolute program");
  let haiku = CatalogModel::new(ModelId::new("haiku").expect("model"), Vec::new());
  connection
    .record_check(ConnectionCheck {
      at: Timestamp::now(),
      result: CheckResult::Available {
        version: "2.1.89".into(),
        sign_in: SignIn::SignedIn,
        catalog: ModelCatalog::new(CatalogSource::Provider, vec![haiku]),
      },
    })
    .expect("check");
  fixture.db.create_connection(&connection).expect("connection");
  let settings = OrchestratorSettings::new(connection.id(), ModelId::new("haiku").expect("model"), None);
  fixture
    .db
    .publish_orchestrator(&settings, Timestamp::now())
    .expect("published");
}

/// A run of a new task `title`, made at `at(start)`, whose agent asked a question and waits for the answer.
fn asking(fixture: &mut Fixture, title: &str, start: i64) -> Run {
  let task = fixture.task(title, at(start));
  let mut run = fixture.started_run(task, at(start + 1));
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let invocation = fixture
    .db
    .start_invocation(
      run.id(),
      execution,
      &ModelId::new("gpt-5.5").expect("model"),
      at(start + 2),
    )
    .expect("invocation")
    .id;
  let question = Message::from_agent(
    run.task(),
    run.id(),
    (execution, invocation),
    AgentMessage::Question,
    MessageBody::new("Keep email sign-in?").expect("text"),
    at(start + 3),
  );
  fixture.db.add_agent_message(&question).expect("asked");
  let events = run
    .wait_step(execution, WaitReason::Message, at(start + 4))
    .expect("waits");
  fixture.record(&run, &events);
  run
}

/// `count` such runs, of tasks ORC-1 onwards.
fn asking_each(fixture: &mut Fixture, count: i64) -> Vec<Run> {
  (0..count)
    .map(|index| asking(fixture, &format!("Task {}", index + 1), 40 + index * 10))
    .collect()
}

/// Follows the project while the Orchestrator drives, its runner attached, at `at(millis)`.
fn follow(fixture: &Fixture, millis: i64) -> Followed {
  let (root, programs) = (fixture.repo.path(), Programs::processes());
  let checks = check_merges_in((root, &programs), &HashSet::new()).expect("checked");
  follow_at(
    (root, fixture.db.project().id(), &programs),
    &checks,
    (Switches { drives: true }, true),
    at(millis),
  )
  .expect("followed")
}

/// The waits of conversation `chat`, with their turns, oldest first.
fn waits_in(fixture: &Fixture, chat: ChatId) -> Vec<(AgentWait, Option<WaitTurn>)> {
  let db = ProjectDb::open(fixture.repo.path()).expect("database");
  db.chat_entries(chat)
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Wait { wait, turn } => Some((wait, turn)),
      _ => None,
    })
    .collect()
}

/// The prompt of the turn `message`.
fn prompt(fixture: &Fixture, message: ChatMessageId) -> String {
  fixture
    .db
    .chat_turn(message)
    .expect("read")
    .expect("the turn")
    .text
    .as_str()
    .to_owned()
}

/// How the turns of `waits` stand, oldest first; `None` for a wait without one.
fn states(waits: &[(AgentWait, Option<WaitTurn>)]) -> Vec<Option<TurnState>> {
  waits
    .iter()
    .map(|(_, turn)| turn.as_ref().map(|turn| turn.state.clone()))
    .collect()
}

#[test]
fn together_one_turn_takes_the_events_that_wait_in_its_conversation() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let mut runs = asking_each(&mut fixture, 3);
  let chat = fixture.everything(at(5));
  let turn = follow(&fixture, 60_000).turn.expect("the oldest event starts a turn");
  assert_eq!(follow(&fixture, 60_000).turn, None, "one turn for the three");
  let kept = waits_in(&fixture, chat.id());
  assert_eq!(states(&kept), vec![Some(TurnState::Thinking); 3]);
  let turns: Vec<&WaitTurn> = kept.iter().filter_map(|(_, turn)| turn.as_ref()).collect();
  assert_eq!((turns[0].message, turns[0].taken_with), (turn.message, None));
  for taken in &turns[1..] {
    assert_eq!(taken.taken_with, Some(turn.message), "taken with the oldest one's turn");
    assert_ne!(taken.message, turn.message, "under a name of its own");
    assert_eq!(taken.answered_by(), turn.message);
  }
  let told = prompt(&fixture, turn.message);
  assert!(
    told.starts_with("[Orchester] 3 events wait in this conversation"),
    "{told}"
  );
  for (index, key) in ["ORC-1", "ORC-2", "ORC-3"].into_iter().enumerate() {
    let event = told.find(&format!("Event {} of 3:", index + 1)).expect("its place");
    assert!(told[event..].contains(key), "{told}");
  }

  // The turn answered: every event it took is answered with it, and none starts another turn by itself.
  assert!(
    fixture
      .db
      .end_chat_turn(turn.message, &TurnState::Answered, at(61_000))
      .expect("ended")
  );
  let kept = waits_in(&fixture, chat.id());
  assert_eq!(states(&kept), vec![Some(TurnState::Answered); 3]);
  assert_eq!(follow(&fixture, 62_000), Followed::default());
  // It settled only the first: the two it did not reach still stand, for the person.
  let execution = runs[0].current_attempt().expect("attempt").executions()[0].id();
  let events = runs[0].resume_step(execution, at(63_000)).expect("resumed");
  fixture.record(&runs[0], &events);
  let view = ChatView::load_in(fixture.repo.path(), chat.id())
    .expect("read")
    .expect("kept");
  assert_eq!(view.standing.len(), 2, "{:?}", view.standing);
  assert_eq!(view.last_turn().map(|(message, _)| message), Some(turn.message));
}

#[test]
fn together_a_turn_takes_five_events_at_most() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  asking_each(&mut fixture, 7);
  let chat = fixture.everything(at(5));
  let first = follow(&fixture, 60_000).turn.expect("a turn");
  let kept = waits_in(&fixture, chat.id());
  let mut expected = vec![Some(TurnState::Thinking); 5];
  expected.extend([None, None]);
  assert_eq!(states(&kept), expected, "the rest wait for the next turn");
  assert!(prompt(&fixture, first.message).contains("Event 5 of 5:"));
  fixture
    .db
    .end_chat_turn(first.message, &TurnState::Answered, at(61_000))
    .expect("ended");
  let second = follow(&fixture, 62_000).turn.expect("the next turn");
  let kept = waits_in(&fixture, chat.id());
  assert_eq!(
    states(&kept)[5..],
    [Some(TurnState::Thinking), Some(TurnState::Thinking)]
  );
  assert_eq!(kept[5].1.as_ref().map(|turn| turn.message), Some(second.message));
  assert_eq!(
    kept[6].1.as_ref().and_then(|turn| turn.taken_with),
    Some(second.message)
  );
  let told = prompt(&fixture, second.message);
  assert!(told.contains("2 events wait") && told.contains("ORC-7"), "{told}");
}

#[test]
fn together_a_turn_that_failed_leaves_each_event_to_be_tried_again() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  asking_each(&mut fixture, 2);
  let chat = fixture.everything(at(5));
  let first = follow(&fixture, 60_000).turn.expect("a turn");
  let failed = TurnState::Failed(TurnFailure::Interrupted);
  fixture
    .db
    .end_chat_turn(first.message, &failed, at(61_000))
    .expect("ended");
  let kept = waits_in(&fixture, chat.id());
  assert_eq!(states(&kept), vec![Some(failed.clone()); 2], "each stays open");
  assert_eq!(fixture.db.automatic_waits_failed().expect("read").len(), 2);
  assert_eq!(follow(&fixture, 62_000).turn, None, "tried again in a minute");
  let again = follow(&fixture, 125_000).turn.expect("the next try");
  assert_ne!(again.message, first.message);
  let kept = waits_in(&fixture, chat.id());
  assert_eq!(states(&kept), vec![Some(TurnState::Thinking); 2]);
  assert_eq!(kept[1].1.as_ref().and_then(|turn| turn.taken_with), Some(again.message));
  let tried: Vec<usize> = fixture
    .db
    .automatic_waits_failed()
    .expect("read")
    .iter()
    .map(|failed| failed.earlier.len())
    .collect();
  assert!(tried.is_empty(), "none is failed while its next turn thinks: {tried:?}");
  let told = prompt(&fixture, again.message);
  assert_eq!(
    told
      .matches("An earlier turn Orchester started for this same event")
      .count(),
    2,
    "each event is told of its failed turn: {told}"
  );
  // A third failure after it would count for each: both keep the first failure among their tries.
  fixture
    .db
    .end_chat_turn(again.message, &failed, at(126_000))
    .expect("ended");
  let earlier: Vec<usize> = fixture
    .db
    .automatic_waits_failed()
    .expect("read")
    .iter()
    .map(|failed| failed.earlier.len())
    .collect();
  assert_eq!(earlier, [1, 1]);
}

#[test]
fn together_a_turn_nobody_took_gives_each_event_back() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  asking_each(&mut fixture, 3);
  let chat = fixture.everything(at(5));
  let first = follow(&fixture, 60_000).turn.expect("a turn");
  assert!(fixture.db.release_wait_turn(first.message).expect("released"));
  assert_eq!(states(&waits_in(&fixture, chat.id())), vec![None; 3]);
  let again = follow(&fixture, 61_000).turn.expect("they start one again");
  assert_eq!(
    states(&waits_in(&fixture, chat.id())),
    vec![Some(TurnState::Thinking); 3]
  );
  // A runner that attaches ends the turn it does not hold, with every event it took.
  let chats = fixture.db.interrupt_chat_turns(&[], None).expect("interrupted");
  assert_eq!(chats, [chat.id()]);
  let interrupted = TurnState::Failed(TurnFailure::Interrupted);
  assert_eq!(states(&waits_in(&fixture, chat.id())), vec![Some(interrupted); 3]);
  // One it holds keeps them all.
  let held = follow(&fixture, 200_000).turn.expect("the next try");
  assert_ne!(held.message, again.message);
  assert!(
    fixture
      .db
      .interrupt_chat_turns(&[held.message], None)
      .expect("held")
      .is_empty()
  );
  assert_eq!(
    states(&waits_in(&fixture, chat.id())),
    vec![Some(TurnState::Thinking); 3]
  );
}

/// A conversation, started at `at(start)`, that watches each of `tasks`.
fn leading(fixture: &mut Fixture, tasks: &[TaskId], start: i64) -> Chat {
  let chat = Chat::named(ChatId::generate(), "Leads", at(start));
  let person = ChatMessageId::generate();
  let text = MessageBody::new("Look after them").expect("text");
  fixture
    .db
    .keep_chat_message(&chat, (person, &text), at(start))
    .expect("kept");
  fixture
    .db
    .end_chat_turn(person, &TurnState::Answered, at(start + 1))
    .expect("ended");
  for task in tasks {
    let watching = Subscription::new(chat.id(), WatchTarget::Task(*task), "", at(start + 2));
    fixture.db.start_subscription(&watching).expect("watching");
  }
  chat
}

/// A task titled `title` whose run succeeded now.
fn succeeded(fixture: &mut Fixture, task: TaskId) -> Run {
  let mut run = fixture.started_run(task, at(50));
  fixture.succeed(&mut run, Timestamp::now());
  run
}

/// Follows the project now, while the Orchestrator drives.
fn follow_now(fixture: &Fixture) -> Followed {
  let (root, programs) = (fixture.repo.path(), Programs::processes());
  let checks = check_merges_in((root, &programs), &HashSet::new()).expect("checked");
  follow_at(
    (root, fixture.db.project().id(), &programs),
    &checks,
    (Switches { drives: true }, true),
    Timestamp::now(),
  )
  .expect("followed")
}

#[test]
fn together_the_successes_of_one_conversation_merge_in_one_turn() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let first = fixture.task("Add login", at(40));
  let second = fixture.task("Add logout", at(41));
  let chat = leading(&mut fixture, &[first, second], 10);
  succeeded(&mut fixture, first);
  succeeded(&mut fixture, second);
  let turn = follow_now(&fixture).turn.expect("the first end's turn");
  let kept = waits_in(&fixture, chat.id());
  assert_eq!(states(&kept), vec![Some(TurnState::Thinking); 2], "{kept:?}");
  assert_eq!(
    kept[1].1.as_ref().and_then(|taken| taken.taken_with),
    Some(turn.message)
  );
}

#[test]
fn together_a_success_is_not_taken_past_a_merge_another_conversation_waits_for() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let first = fixture.task("Add login", at(40));
  let second = fixture.task("Add logout", at(41));
  let third = fixture.task("Add sign-up", at(42));
  let ours = leading(&mut fixture, &[first, third], 10);
  let theirs = leading(&mut fixture, &[second], 20);
  // The runs end in this order: ours, theirs, ours.
  succeeded(&mut fixture, first);
  succeeded(&mut fixture, second);
  succeeded(&mut fixture, third);
  let turn = follow_now(&fixture).turn.expect("the first end's turn");
  assert_eq!(turn.chat, ours.id());
  let kept = waits_in(&fixture, ours.id());
  assert_eq!(
    states(&kept),
    vec![Some(TurnState::Thinking), None],
    "the third run's merge follows the second's, which another conversation waits for"
  );
  assert_eq!(follow_now(&fixture).turn, None, "and that one waits for the first");
  assert_eq!(states(&waits_in(&fixture, theirs.id())), vec![None]);
  fixture
    .db
    .end_chat_turn(turn.message, &TurnState::Answered, Timestamp::now())
    .expect("ended");
  let next = follow_now(&fixture).turn.expect("the second end's turn");
  assert_eq!(next.chat, theirs.id());
}
