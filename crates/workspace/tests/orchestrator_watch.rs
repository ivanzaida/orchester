//! The person's watch commands, applied by the app itself (ORC-760, `orchestrator::watch`): /watch keeps new targets
//! at once and returns those another conversation leads for the card, keeping them nowhere; *Take it here* moves the
//! subscription and writes the line in the conversation that lost it; a task inside a module another conversation
//! leads writes its line there, with no card; a stop cancels nothing and lists what was open; a target closed or
//! deleted meanwhile is refused; and all of it is kept as the conversation's entries while a turn is under way. The
//! agent's own `watch` and `unwatch` are `orchestrator_watch_tools`'; taking over is no tool of its.
//! `cargo test -p orchester-workspace --test orchestrator_watch watch_`.

mod support;

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, ModuleId, TaskId},
  module::{ModuleDetails, ModuleState},
  name::Name,
  orchestrator::{
    Chat, ChatEvent, EntryKind, TurnState, WatchTarget,
    watch::{NamedTarget, OpenTask, OpenWork, WatchCommand, WatchLine},
  },
  run::RunStatus,
  store::{RunStore, TaskStore},
  task::organization::ChangeActor,
  time::Timestamp,
};
use orchester_workspace::orchestrator::watch::{
  StopBy, StopWhat, WatchError, leave_there_in, stop_in, take_over_in, watch_in, watching_in,
};
use support::{Fixture, at};

/// A conversation that said `text`, its turn answered unless `thinking`.
fn conversation(fixture: &mut Fixture, text: &str, thinking: bool) -> Chat {
  let body = MessageBody::new(text).expect("text");
  let chat = Chat::start(ChatId::generate(), &body, Timestamp::now());
  let message = ChatMessageId::generate();
  assert!(
    fixture
      .db
      .keep_chat_message(&chat, (message, &body), Timestamp::now())
      .expect("kept")
  );
  if !thinking {
    fixture
      .db
      .end_chat_turn(message, &TurnState::Answered, Timestamp::now())
      .expect("ended");
  }
  chat
}

fn module(fixture: &mut Fixture, name: &str) -> ModuleId {
  let details =
    ModuleDetails::new(Name::new(name).expect("name"), "", (None, None), ModuleState::Open).expect("details");
  fixture.db.create_module(details, at(10)).expect("module").id()
}

fn in_module(fixture: &mut Fixture, title: &str, module: ModuleId) -> TaskId {
  let task = fixture.task(title, at(20));
  fixture
    .db
    .set_task_module(task, Some(module), ChangeActor::Person, at(30))
    .expect("moved");
  task
}

fn named(target: WatchTarget, name: &str) -> NamedTarget {
  NamedTarget {
    target,
    name: Some(name.to_owned()),
  }
}

fn key(fixture: &Fixture, task: TaskId) -> String {
  fixture.db.task(task).expect("read").expect("task").key().to_string()
}

/// The watch entries of `chat`, in order.
fn watch_entries(fixture: &Fixture, chat: ChatId) -> Vec<ChatEvent> {
  fixture
    .db
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Event(event @ (ChatEvent::WatchCommand { .. } | ChatEvent::Watch { .. })) => Some(event),
      _ => None,
    })
    .collect()
}

fn leader(fixture: &Fixture, target: WatchTarget) -> Option<ChatId> {
  fixture
    .db
    .active_subscriptions()
    .expect("subscriptions")
    .of(target)
    .map(|led| led.chat())
}

#[test]
fn watch_several_targets_keeps_the_new_and_returns_the_one_already_led() {
  let mut fixture = Fixture::new();
  let delivery = WatchTarget::Module(module(&mut fixture, "Delivery"));
  let billing = WatchTarget::Module(module(&mut fixture, "Billing"));
  let theirs = conversation(&mut fixture, "Billing work", false);
  watch_in(fixture.repo.path(), &theirs, &[named(billing, "Billing")], "").expect("watched");
  let since = fixture
    .db
    .active_subscriptions()
    .expect("read")
    .of(billing)
    .expect("led")
    .since();
  let mine = conversation(&mut fixture, "Delivery work", false);

  let targets = [named(delivery, "Delivery"), named(billing, "Billing")];
  let watched = watch_in(fixture.repo.path(), &mine, &targets, "  Ask before merging. ").expect("watched");

  assert_eq!(watched.started, vec![named(delivery, "Delivery")]);
  assert_eq!(watched.led_elsewhere.len(), 1);
  let card = &watched.led_elsewhere[0];
  assert_eq!(
    (card.target.clone(), card.chat, card.since),
    (named(billing, "Billing"), theirs.id(), since)
  );
  assert_eq!(card.title, "Billing work");
  assert_eq!(leader(&fixture, delivery), Some(mine.id()));
  assert_eq!(
    leader(&fixture, billing),
    Some(theirs.id()),
    "the one led elsewhere is kept nowhere"
  );
  assert_eq!(
    watch_entries(&fixture, mine.id()),
    vec![
      ChatEvent::WatchCommand {
        command: WatchCommand::Watch {
          targets: targets.to_vec(),
          order: "Ask before merging.".to_owned(),
        },
      },
      ChatEvent::Watch {
        line: WatchLine::Watching {
          started: vec![named(delivery, "Delivery")],
          already_here: Vec::new(),
          led_elsewhere: watched.led_elsewhere.clone(),
          gone: Vec::new(),
          at_request: None,
        },
      },
    ]
  );
}

#[test]
fn watch_take_over_moves_the_subscription_and_writes_both_lines() {
  let mut fixture = Fixture::new();
  let billing = WatchTarget::Module(module(&mut fixture, "Billing"));
  let theirs = conversation(&mut fixture, "Billing work", false);
  watch_in(fixture.repo.path(), &theirs, &[named(billing, "Billing")], "").expect("watched");
  let mine = conversation(&mut fixture, "Delivery work", false);

  let taken = take_over_in(
    fixture.repo.path(),
    &mine,
    &named(billing, "Billing"),
    "Merge when green.",
  )
  .expect("taken");

  assert_eq!(taken.from, Some(theirs.id()));
  assert_eq!(leader(&fixture, billing), Some(mine.id()));
  assert_eq!(
    watch_entries(&fixture, mine.id()).last(),
    Some(&ChatEvent::Watch {
      line: WatchLine::TookOver {
        target: named(billing, "Billing"),
        from: theirs.id(),
        title: "Billing work".to_owned(),
      },
    })
  );
  assert_eq!(
    watch_entries(&fixture, theirs.id()).last(),
    Some(&ChatEvent::Watch {
      line: WatchLine::NowLedIn {
        target: named(billing, "Billing"),
        chat: mine.id(),
        title: "Delivery work".to_owned(),
        taken_over: true,
      },
    })
  );
  assert!(matches!(
    take_over_in(fixture.repo.path(), &mine, &named(billing, "Billing"), ""),
    Err(WatchError::AlreadyHere)
  ));
}

#[test]
fn watch_leave_there_keeps_the_line_and_moves_nothing() {
  let mut fixture = Fixture::new();
  let billing = WatchTarget::Module(module(&mut fixture, "Billing"));
  let theirs = conversation(&mut fixture, "Billing work", false);
  watch_in(fixture.repo.path(), &theirs, &[named(billing, "Billing")], "").expect("watched");
  let mine = conversation(&mut fixture, "Delivery work", false);
  let watched = watch_in(fixture.repo.path(), &mine, &[named(billing, "Billing")], "").expect("watched");
  let theirs_before = watch_entries(&fixture, theirs.id());

  leave_there_in(fixture.repo.path(), &mine, &watched.led_elsewhere[0]).expect("left");

  assert_eq!(leader(&fixture, billing), Some(theirs.id()));
  assert_eq!(
    watch_entries(&fixture, mine.id()).last(),
    Some(&ChatEvent::Watch {
      line: WatchLine::LeftThere {
        target: named(billing, "Billing"),
        chat: theirs.id(),
        title: "Billing work".to_owned(),
      },
    })
  );
  assert_eq!(
    watch_entries(&fixture, theirs.id()),
    theirs_before,
    "that conversation is told nothing"
  );
}

#[test]
fn watch_task_inside_a_module_led_elsewhere_writes_the_line_there_with_no_card() {
  let mut fixture = Fixture::new();
  let webhooks = module(&mut fixture, "Webhooks");
  let task = in_module(&mut fixture, "Retry failed deliveries", webhooks);
  let theirs = conversation(&mut fixture, "Webhooks work", false);
  watch_in(
    fixture.repo.path(),
    &theirs,
    &[named(WatchTarget::Module(webhooks), "Webhooks")],
    "",
  )
  .expect("watched");
  let mine = conversation(&mut fixture, "Retries", false);
  let task_key = key(&fixture, task);

  let watched = watch_in(
    fixture.repo.path(),
    &mine,
    &[named(WatchTarget::Task(task), &task_key)],
    "",
  )
  .expect("watched");

  assert_eq!(watched.started, vec![named(WatchTarget::Task(task), &task_key)]);
  assert!(watched.led_elsewhere.is_empty(), "no card: the task subscription wins");
  assert_eq!(leader(&fixture, WatchTarget::Module(webhooks)), Some(theirs.id()));
  assert_eq!(
    watch_entries(&fixture, theirs.id()).last(),
    Some(&ChatEvent::Watch {
      line: WatchLine::NowLedIn {
        target: named(WatchTarget::Task(task), &task_key),
        chat: mine.id(),
        title: "Retries".to_owned(),
        taken_over: false,
      },
    })
  );
}

#[test]
fn watch_stop_cancels_nothing_and_lists_the_open_runs() {
  let mut fixture = Fixture::new();
  let webhooks = module(&mut fixture, "Webhooks");
  let running = in_module(&mut fixture, "Retry failed deliveries", webhooks);
  let run = fixture.started_run(running, at(40));
  let queued = fixture.queued_task("Sign deliveries", at(41));
  fixture
    .db
    .set_task_module(queued, Some(webhooks), ChangeActor::Person, at(42))
    .expect("moved");
  in_module(&mut fixture, "Document the retries", webhooks);
  let mine = conversation(&mut fixture, "Webhooks work", false);
  watch_in(
    fixture.repo.path(),
    &mine,
    &[named(WatchTarget::Module(webhooks), "Webhooks")],
    "",
  )
  .expect("watched");

  let unwatched = stop_in(fixture.repo.path(), &mine, &StopWhat::All, StopBy::Unwatch).expect("stopped");

  let left_open = vec![
    OpenTask {
      task: key(&fixture, running),
      work: OpenWork::Runs,
    },
    OpenTask {
      task: key(&fixture, queued),
      work: OpenWork::Queued,
    },
  ];
  assert_eq!(unwatched.left_open, left_open, "the draft is not open work");
  assert_eq!(leader(&fixture, WatchTarget::Module(webhooks)), None);
  let status = fixture.db.run(run.id()).expect("read").expect("run").status().clone();
  assert_eq!(status, *run.status(), "the run goes on as it was");
  assert!(matches!(status, RunStatus::Running));
  let entries = watch_entries(&fixture, mine.id());
  assert_eq!(
    entries[entries.len() - 2..],
    [
      ChatEvent::WatchCommand {
        command: WatchCommand::UnwatchAll,
      },
      ChatEvent::Watch {
        line: WatchLine::Stopped {
          targets: vec![named(WatchTarget::Module(webhooks), "Webhooks")],
          left_open,
          at_request: None,
        },
      },
    ]
  );
  assert!(matches!(
    stop_in(fixture.repo.path(), &mine, &StopWhat::All, StopBy::Chip),
    Err(WatchError::NotWatched)
  ));
}

#[test]
fn watch_refuses_a_target_that_vanished_meanwhile() {
  let mut fixture = Fixture::new();
  let mobile = module(&mut fixture, "Mobile release");
  let read = fixture.db.module(mobile).expect("read").expect("module");
  let closed = ModuleDetails::new(
    Name::new("Mobile release").expect("name"),
    "",
    (None, None),
    ModuleState::Closed,
  )
  .expect("details");
  fixture
    .db
    .update_module(mobile, read.details(), closed)
    .expect("closed");
  let task = fixture.task("Retry failed deliveries", at(20));
  let task_key = key(&fixture, task);
  fixture.db.delete_task(task, at(30)).expect("deleted");
  let mine = conversation(&mut fixture, "Releases", false);
  let gone = [
    named(WatchTarget::Module(mobile), "Mobile release"),
    named(WatchTarget::Task(task), &task_key),
  ];

  let refused = watch_in(fixture.repo.path(), &mine, &gone, "").expect_err("refused");

  assert!(matches!(&refused, WatchError::Gone(targets) if targets.as_slice() == gone));
  let message = refused.message();
  assert_eq!(message.key, "orchestrator.watch.gone");
  assert_eq!(message.args, vec![("targets", format!("Mobile release, {task_key}"))]);
  assert!(watch_entries(&fixture, mine.id()).is_empty(), "nothing is kept");
  assert!(fixture.db.active_subscriptions().expect("read").as_slice().is_empty());
  assert!(matches!(
    take_over_in(fixture.repo.path(), &mine, &gone[1], ""),
    Err(WatchError::Gone(_))
  ));
}

#[test]
fn watch_applies_while_a_turn_of_the_conversation_runs() {
  let mut fixture = Fixture::new();
  let delivery = WatchTarget::Module(module(&mut fixture, "Delivery"));
  let mine = conversation(&mut fixture, "What is left in Delivery?", true);

  watch_in(fixture.repo.path(), &mine, &[named(delivery, "Delivery")], "").expect("watched while it answers");

  let entries = fixture.db.chat_entries(mine.id()).expect("entries");
  assert!(
    matches!(
      &entries[0].kind,
      EntryKind::Person {
        turn: TurnState::Thinking,
        ..
      }
    ),
    "the turn under way goes on"
  );
  assert!(matches!(
    entries[1].kind,
    EntryKind::Event(ChatEvent::WatchCommand { .. })
  ));
  assert!(matches!(entries[2].kind, EntryKind::Event(ChatEvent::Watch { .. })));
  assert_eq!(entries.len(), 3, "no turn of its own");
  assert_eq!(leader(&fixture, delivery), Some(mine.id()));
}

/// `/watching` (ORC-775) keeps only the person's command: the answer under it is read live, and no turn starts.
#[test]
fn watch_watching_keeps_the_command_only_while_the_conversation_watches_something() {
  let mut fixture = Fixture::new();
  let delivery = WatchTarget::Module(module(&mut fixture, "Delivery"));
  let mine = conversation(&mut fixture, "Delivery work", false);
  assert!(matches!(
    watching_in(fixture.repo.path(), &mine),
    Err(WatchError::NotWatched)
  ));
  assert!(watch_entries(&fixture, mine.id()).is_empty(), "nothing is kept");
  watch_in(fixture.repo.path(), &mine, &[named(delivery, "Delivery")], "").expect("watched");

  watching_in(fixture.repo.path(), &mine).expect("asked");

  let entries = fixture.db.chat_entries(mine.id()).expect("entries");
  assert_eq!(
    entries.last().map(|entry| &entry.kind),
    Some(&EntryKind::Event(ChatEvent::WatchCommand {
      command: WatchCommand::Watching,
    }))
  );
  assert_eq!(
    entries.len(),
    4,
    "the message, the /watch and its line, and /watching: no turn of its own"
  );
}

/// ORC-893: the agent watches and stops watching at the person's request with `watch` and `unwatch` only; taking a
/// target over stays the person's click on the card.
#[test]
fn watch_take_over_is_no_tool_of_the_agent() {
  for tool in orchestrator_workspace_tools() {
    let watch_tool = tool == "watch" || tool == "unwatch";
    assert!(
      watch_tool || (!tool.contains("watch") && !tool.contains("take_over") && !tool.contains("subscri")),
      "{tool} lets the agent change what the conversation watches"
    );
  }
}

/// The tools of the chat that change something, while it drives (it reads the watches with `get_watches`).
fn orchestrator_workspace_tools() -> Vec<String> {
  orchester_workspace::orchestrator::tools::listed(true)
    .into_iter()
    .filter(|tool| tool.annotations.as_ref().and_then(|hints| hints.read_only_hint) != Some(true))
    .map(|tool| tool.name.to_string())
    .collect()
}
