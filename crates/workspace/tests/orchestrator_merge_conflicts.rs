//! A merge of a task's run that stopped at conflicts is an event of its task (ORC-768,
//! `orchestrator::merge_conflicts`): the person's merge, `merge_run`'s and the merge queue's raise one event per
//! attempt, kept in the conversation that leads the task, and none from before the chat followed the project's runs;
//! the Orchestrator's own merge in the turn that merges a run on its own keeps its proposal and starts no second turn,
//! even when the follower looks while that turn still thinks.
//! `cargo test -p orchester-workspace --test orchestrator_merge_conflicts merge_conflict`.

mod support;

use std::collections::HashSet;

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
  },
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, ConnectionId},
  integration::attempt::{AttemptOutcome, MergeAsker, MergeAttempt, MergeVia},
  merge_queue::{Left, QueueEvent, QueueState},
  orchestrator::{
    ActionOutcome, AgentWait, Chat, ChatAction, EntryKind, OrchestratorSettings, Subscription, TaskEvent,
    TaskEventKind, TurnState, WaitKind, WaitTurn, WatchTarget,
    change::{Change, ProposedChange},
  },
  resource::ModelId,
  run::Run,
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_workspace::orchestrator::waits::{Followed, Switches, check_merges_in, follow_in};
use support::{Fixture, at};

/// *Answer agents* on.
const ANSWERING: Switches = Switches {
  answers: true,
  continues: false,
  merges: false,
};

/// *Answer agents*, and *merge finished runs* with *merge runs*, on.
const MERGING: Switches = Switches {
  answers: true,
  continues: false,
  merges: true,
};

/// A checked, signed-in Claude Code connection, published as what the Orchestrator runs on.
fn ready(fixture: &mut Fixture) {
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    orchester_core::name::Name::new("Claude · Kontur").expect("name"),
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

/// Follows the waits with the project's runner attached, under `switches`.
fn follow(fixture: &Fixture, switches: Switches) -> Followed {
  let (root, programs) = (fixture.repo.path(), Programs::processes());
  let checks = check_merges_in((root, &programs), &HashSet::new()).expect("checked");
  follow_in((root, fixture.db.project().id(), &programs), &checks, (switches, true)).expect("followed")
}

/// The waits of conversation `chat`, with their turns.
fn waits_in(fixture: &Fixture, chat: ChatId) -> Vec<(AgentWait, Option<WaitTurn>)> {
  fixture
    .db
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Wait { wait, turn } => Some((wait, turn)),
      _ => None,
    })
    .collect()
}

/// The merge-conflict events of conversation `chat`, with their turns.
fn conflicts_in(fixture: &Fixture, chat: ChatId) -> Vec<(AgentWait, Option<WaitTurn>)> {
  waits_in(fixture, chat)
    .into_iter()
    .filter(|(wait, _)| {
      matches!(
        wait.kind,
        WaitKind::Event(TaskEvent {
          kind: TaskEventKind::MergeConflict,
          ..
        })
      )
    })
    .collect()
}

/// A conversation, started at `at(start)`, whose answered turn updated task `key`.
fn dealt_with(fixture: &mut Fixture, key: &str, start: i64) -> Chat {
  let chat = Chat::named(ChatId::generate(), key, at(start));
  let person = ChatMessageId::generate();
  let text = MessageBody::new("Look after it").expect("text");
  fixture
    .db
    .keep_chat_message(&chat, (person, &text), at(start))
    .expect("kept");
  let action = ChatAction::new("update_task", Some(key.to_owned()), ActionOutcome::Done);
  fixture
    .db
    .add_chat_action(chat.id(), &action, at(start + 1))
    .expect("kept");
  fixture
    .db
    .end_chat_turn(person, &TurnState::Answered, at(start + 2))
    .expect("ended");
  chat
}

/// A run of a new task titled `title`, which succeeded now.
fn succeeded(fixture: &mut Fixture, title: &str) -> Run {
  let task = fixture.task(title, at(40));
  let mut run = fixture.started_run(task, at(50));
  fixture.succeed(&mut run, Timestamp::now());
  run
}

/// `asked` tried to merge `run` at `when`, and the merge stopped at conflicts in `src/login.rs`.
fn conflicted(fixture: &mut Fixture, run: &Run, asked: (MergeAsker, MergeVia), when: Timestamp) {
  let outcome = AttemptOutcome::Conflicts {
    files: vec!["src/login.rs".to_owned()],
  };
  let attempt = MergeAttempt::new((run.id(), when), asked, outcome, None);
  fixture.db.record_merge_attempt(&attempt).expect("recorded");
}

const PERSON: (MergeAsker, MergeVia) = (MergeAsker::Person, MergeVia::TaskPage);
const MERGE_RUN: (MergeAsker, MergeVia) = (MergeAsker::Automation, MergeVia::MergeRun);

/// Ends the turn under `message` as answered.
fn answered(fixture: &mut Fixture, message: ChatMessageId) {
  fixture
    .db
    .end_chat_turn(message, &TurnState::Answered, Timestamp::now())
    .expect("ended");
}

#[test]
fn merge_conflict_raises_one_event_per_attempt_in_the_leading_conversation() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let leading = dealt_with(&mut fixture, "Look after ORC-1", 10);
  let run = succeeded(&mut fixture, "Add login");
  let watching = Subscription::new(leading.id(), WatchTarget::Task(run.task()), "", at(11));
  fixture.db.start_subscription(&watching).expect("watching");
  // Another conversation deals with ORC-1 after the subscription, the newer one.
  let elsewhere = dealt_with(&mut fixture, "ORC-1", 20);
  let ended = follow(&fixture, ANSWERING)
    .turn
    .expect("the run's success starts its turn");
  answered(&mut fixture, ended.message);
  // A conflict from before the chat followed the project's runs is history.
  conflicted(&mut fixture, &run, PERSON, at(60));
  assert_eq!(follow(&fixture, ANSWERING), Followed::default());

  conflicted(&mut fixture, &run, PERSON, Timestamp::now());
  let turn = follow(&fixture, ANSWERING)
    .turn
    .expect("the conflict starts a turn by itself");
  assert_eq!(turn.chat, leading.id(), "in the leading conversation");
  assert!(conflicts_in(&fixture, elsewhere.id()).is_empty(), "nowhere else");
  let events = conflicts_in(&fixture, leading.id());
  assert_eq!(events.len(), 1, "{events:?}");
  let (event, started) = &events[0];
  assert_eq!((event.run, event.task.as_str()), (run.id(), "ORC-1"));
  assert!(event.automatic && !event.own, "{event:?}");
  assert!(event.link.ends_with("/summary"), "{}", event.link);
  assert_eq!(started.as_ref().map(|started| started.message), Some(turn.message));
  let prompt = fixture
    .db
    .chat_turn(turn.message)
    .expect("read")
    .expect("the turn")
    .text;
  assert!(prompt.as_str().contains("hit conflicts"), "{}", prompt.as_str());
  answered(&mut fixture, turn.message);
  assert_eq!(
    follow(&fixture, ANSWERING),
    Followed::default(),
    "one event per attempt"
  );

  conflicted(&mut fixture, &run, MERGE_RUN, Timestamp::now());
  follow(&fixture, ANSWERING)
    .turn
    .expect("the next attempt is an event of its own");
  assert_eq!(conflicts_in(&fixture, leading.id()).len(), 2);
}

#[test]
fn merge_conflict_the_queue_left_for_a_person_is_an_event_of_its_task() {
  let mut fixture = Fixture::new();
  let chat = dealt_with(&mut fixture, "ORC-1", 10);
  let run = succeeded(&mut fixture, "Add login");
  let watching = Subscription::new(chat.id(), WatchTarget::Task(run.task()), "", at(11));
  fixture.db.start_subscription(&watching).expect("watching");
  let now = Timestamp::now();
  fixture
    .db
    .join_merge_queue((run.id(), run.task()), now, now)
    .expect("joined");
  let mut entry = fixture.db.merge_queue_entry(run.id()).expect("read").expect("queued");
  let left = Left::Conflicts {
    files: vec!["src/login.rs".to_owned()],
  };
  entry.state = QueueState::Left(left.clone());
  entry.since = Timestamp::now();
  let saved = fixture
    .db
    .save_merge_queue_entry(&entry, &QueueState::Queued, (&[QueueEvent::Left(left)], entry.since))
    .expect("saved");
  assert!(saved);
  assert!(follow(&fixture, Switches::default()).kept);
  let events = conflicts_in(&fixture, chat.id());
  assert_eq!(events.len(), 1, "{events:?}");
  assert_eq!(events[0].0.run, run.id());
  assert!(!follow(&fixture, Switches::default()).kept, "kept once");
}

#[test]
fn merge_conflict_of_the_orchestrators_own_merge_starts_no_second_turn() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  fixture.everything(at(5));
  let run = succeeded(&mut fixture, "Add login");
  let turn = follow(&fixture, MERGING)
    .turn
    .expect("the turn that merges the run on its own");
  // Its merge_run stops at conflicts; the follower looks before the turn kept its action.
  conflicted(&mut fixture, &run, MERGE_RUN, Timestamp::now());
  assert!(!follow(&fixture, MERGING).kept, "the conflict waits for the turn");
  assert!(conflicts_in(&fixture, turn.chat).is_empty());
  let merge = Change::Merge {
    run: run.id(),
    number: run.number().get(),
    checkpoint: None,
    into: None,
  };
  let refused = ActionOutcome::Refused {
    code: "merge_failed".to_owned(),
    reason: "Merging stopped at conflicts in src/login.rs.".to_owned(),
  };
  let action = ChatAction {
    change: Some(Box::new(ProposedChange {
      subject: "Add login".to_owned(),
      change: merge,
    })),
    ..ChatAction::new("merge_run", Some("ORC-1".to_owned()), refused)
  };
  fixture
    .db
    .add_chat_action(turn.chat, &action, Timestamp::now())
    .expect("kept");
  answered(&mut fixture, turn.message);

  let followed = follow(&fixture, MERGING);
  assert!(followed.kept, "the conflict is kept once the turn ended");
  assert_eq!(followed.turn, None, "no second turn");
  let events = conflicts_in(&fixture, turn.chat);
  assert_eq!(events.len(), 1, "{events:?}");
  let (event, started) = &events[0];
  assert!(event.own && !event.automatic, "{event:?}");
  assert_eq!(*started, None);
  assert_eq!(follow(&fixture, MERGING), Followed::default());
}
