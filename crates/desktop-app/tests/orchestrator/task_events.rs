//! Events of tasks' runs (ORC-755) in the conversation (ORC-779; `Parts · Lines` e `c9YRv` and `04 · Leading two
//! modules` `OKtX2` in `Orchestrator watch · redesign`): each one bordered line 34 high with what happened, the task
//! and the run, the task's module and its one link; a task the run waits on is named by its key and its run. The
//! answer of a turn an event or a wait started by itself carries a tag of what started it. A stall settles in the
//! open conversation once its run progresses, and a tool request that came to the person is a wait line (ORC-894).

use orchester_app::routes::OrchestratorChat;
use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  module::{ModuleDetails, ModuleState},
  name::Name,
  orchestrator::{AgentWait, Chat, EntryKind, TaskEvent, TaskEventKind, TurnState, WaitKind},
  resource::ModelId,
  run::Run,
  store::{ActivityStore, TaskStore},
  task::organization::ChangeActor,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::display;

use super::{
  harness::{Orchestrator, Setup},
  standing::{entry_of, follow, started, waiting},
};

/// The wait of `kind` of `run`, of task `task`.
fn wait_of(kind: WaitKind, run: &Run, task: &str) -> AgentWait {
  AgentWait {
    kind,
    task: task.to_owned(),
    title: format!("Work of {task}"),
    run: run.id(),
    number: run.number().get(),
    step: None,
    place: None,
    others: 0,
    agent: Some("Builder".to_owned()),
    link: format!("/tasks/{task}"),
    own: false,
    automatic: true,
    continuing: None,
    merging: None,
  }
}

/// The key of `run`'s task.
fn key_of(db: &ProjectDb, run: &Run) -> String {
  db.task(run.task()).expect("read").expect("the task").key().to_string()
}

/// Keeps `wait` in `chat`; its entry.
fn keep(db: &mut ProjectDb, chat: &Chat, (key, wait): (&str, &AgentWait)) -> i64 {
  assert!(db.keep_agent_wait(chat, (key, wait), Timestamp::now()).expect("kept"));
  db.chat_entries(chat.id())
    .expect("entries")
    .into_iter()
    .rev()
    .find_map(|entry| matches!(entry.kind, EntryKind::Wait { .. }).then_some(entry.seq))
    .expect("the wait's entry")
}

/// Gives wait `seq` of `chat` a turn that says `said` and ends; the turn's message.
fn turned(db: &mut ProjectDb, (chat, seq): (ChatId, i64), said: &str) -> ChatMessageId {
  let message = ChatMessageId::generate();
  assert!(db.start_wait_turn((chat, seq), message).expect("started"));
  db.add_chat_reply(chat, (said, &ModelId::new("haiku").expect("model")), Timestamp::now())
    .expect("reply");
  db.end_chat_turn(message, &TurnState::Answered, Timestamp::now())
    .expect("ended");
  message
}

/// Puts `run`'s task in a new module, *Webhooks v2*.
fn in_module(db: &mut ProjectDb, run: &Run) {
  let name = Name::new("Webhooks v2").expect("name");
  let details = ModuleDetails::new(name, "", (None, None), ModuleState::Open).expect("details");
  let module = db.create_module(details, Timestamp::now()).expect("module");
  db.set_task_module(run.task(), Some(module.id()), ChangeActor::Person, Timestamp::now())
    .expect("moved");
}

/// A conversation the person started, answered.
fn conversation(db: &mut ProjectDb) -> Chat {
  let (chat, at) = (ChatId::generate(), Timestamp::now());
  let body = MessageBody::new("Watch Webhooks v2").expect("text");
  let first = ChatMessageId::generate();
  db.keep_chat_message(&Chat::start(chat, &body, at), (first, &body), at)
    .expect("kept");
  db.end_chat_turn(first, &TurnState::Answered, at).expect("ended");
  db.chat(chat).expect("read").expect("the conversation")
}

#[test]
fn orchestrator_task_events_are_bordered_lines_and_their_turns_say_what_started_them() {
  let mut screen = Orchestrator::new(Setup::NoRunner);
  let delivery = screen.delivery();
  let (waits, _) = waiting(&screen, delivery, "Retry failed deliveries");
  let (waited, _) = waiting(&screen, delivery, "Shared rate limiter");
  let (asks, _) = waiting(&screen, delivery, "Sign in");
  let mut db = screen.db();
  let (key, waited_key, asks_key) = (key_of(&db, &waits), key_of(&db, &waited), key_of(&db, &asks));
  in_module(&mut db, &waits);
  let chat = conversation(&mut db);
  let event = |kind| {
    WaitKind::Event(TaskEvent {
      kind,
      at: Timestamp::now(),
    })
  };
  let stalled = event(TaskEventKind::NoProgress {
    since: Timestamp::now(),
    minutes: 20,
  });
  let stall = keep(&mut db, &chat, ("stall", &wait_of(stalled, &waits, &key)));
  let merged = event(TaskEventKind::WaitedMerged {
    task: waited.task(),
    run: waited.id(),
  });
  let merge = keep(&mut db, &chat, ("merged", &wait_of(merged, &waits, &key)));
  let merge_turn = turned(&mut db, (chat.id(), merge), "KON-41 waited on this.");
  let asked = keep(&mut db, &chat, ("asked", &wait_of(WaitKind::Message, &asks, &asks_key)));
  let asked_turn = turned(&mut db, (chat.id(), asked), "Answered from the docs.");
  drop(db);

  screen.go(OrchestratorChat::Chat(chat.id()));
  let (stall, merge) = (
    format!("orchestrator-wait-wait-{stall}"),
    format!("orchestrator-wait-{merge_turn}"),
  );
  screen.until("the event lines", |screen| {
    screen.exists(&stall) && screen.exists(&merge)
  });
  for line in [&stall, &merge] {
    assert!((screen.bounds(line).height - 34.0).abs() < 0.5, "{line} is one 34 line");
    assert_eq!(screen.text(&format!("{line}-module")), "Webhooks v2", "{line}");
  }
  // Its run waits for a message, and made no progress since: the stall is past, and says so (ORC-894).
  assert_eq!(
    screen.text(&format!("{stall}-title")),
    "No progress for 20 min · waits now"
  );
  assert_eq!(
    screen.text(&format!("{stall}-target")),
    format!("{key} · #{}", waits.number().get())
  );
  assert_eq!(screen.text(&format!("{stall}-open")), "Run ↗");
  assert_eq!(screen.text(&format!("{merge}-title")), "Merged into the base branch");
  assert_eq!(
    screen.text(&format!("{merge}-target")),
    format!("{waited_key} · #{}", waited.number().get()),
    "the task the run waited on and its run"
  );
  assert_eq!(screen.text(&format!("{merge}-open")), "Changes ↗");
  assert_eq!(
    screen.text(&format!("orchestrator-started-by-{merge_turn}-label")),
    format!("{waited_key} merged")
  );
  assert_eq!(
    screen.text(&format!("orchestrator-started-by-{asked_turn}-label")),
    format!("{asks_key}'s wait"),
    "an agent's wait says so on the answer nested under it"
  );
  assert!(
    screen.exists(&format!("orchestrator-wait-{asked_turn}-chip")),
    "a wait keeps its line and its chip"
  );
}

#[test]
fn orchestrator_task_events_a_stall_settles_in_the_open_conversation_once_its_run_progresses() {
  let mut screen = Orchestrator::new(Setup::NoRunner);
  let delivery = screen.delivery();
  let hour_ago = Timestamp::from_unix_millis(Timestamp::now().unix_millis() - 60 * 60_000);
  let (run, execution) = started(&screen, (delivery, "Retry failed deliveries"), hour_ago);
  follow(&screen);
  let seq = entry_of(&screen, &run);
  screen.go(OrchestratorChat::Latest);
  let title = format!("orchestrator-wait-wait-{seq}-title");
  screen.until("the stall", |screen| screen.exists(&title));
  assert_eq!(screen.text(&title), "No progress for 20 min", "it stands");
  // The agent is launched again: only the run's transcript tells, no run changes state.
  let now = Timestamp::now();
  screen
    .db()
    .start_invocation(run.id(), execution, &ModelId::new("haiku").expect("model"), now)
    .expect("launched");
  let services = screen.window.launch.services().expect("the session started");
  services.runners.count_change_for_test(&screen.root, false);
  let moving = format!("No progress for 20 min · moving again {}", display::clock(now));
  screen.until("the stall settled", |screen| screen.text(&title) == moving);
}

#[test]
fn orchestrator_task_events_a_tool_request_is_a_wait_line_for_the_person() {
  let mut screen = Orchestrator::new(Setup::NoRunner);
  let delivery = screen.delivery();
  let (asks, _) = waiting(&screen, delivery, "Sign in");
  let mut db = screen.db();
  let key = key_of(&db, &asks);
  let chat = conversation(&mut db);
  let approval = AgentWait {
    automatic: false,
    ..wait_of(WaitKind::Approval, &asks, &key)
  };
  let seq = keep(&mut db, &chat, ("approval", &approval));
  drop(db);
  screen.go(OrchestratorChat::Chat(chat.id()));
  let line = format!("orchestrator-wait-wait-{seq}");
  screen.until("the tool request's line", |screen| screen.exists(&line));
  assert_eq!(
    screen.text(&format!("{line}-title")),
    "Builder waits for a tool approval"
  );
  assert_eq!(
    screen.text(&format!("{line}-target")),
    format!("{key} · #{}", asks.number().get())
  );
  assert_eq!(
    screen.text(&format!("{line}-open")),
    "Open ↗",
    "to its card on the task's Conversation"
  );
}
