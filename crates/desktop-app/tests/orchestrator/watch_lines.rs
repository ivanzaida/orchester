//! What the app writes in a conversation about what it watches (ORC-777; `04 · Leading two modules` `OKtX2` and
//! `Parts · Lines` `avDZO` a–d in `Orchestrator watch · redesign`): the person's command as a bubble with its chips,
//! its order and *Applied*; the subscription line with how many tasks the module holds; *… is now led in «…»* with the
//! module the task leaves and *Open ↗*; the stop line with what it left open as links; and the line of a subscription
//! that ended by itself. Each is a row of its own, with no turn.

use orchester_app::routes::OrchestratorChat;
use orchester_core::{
  id::{ChatId, ModuleId},
  module::{ModuleDetails, ModuleState},
  name::Name,
  orchestrator::{
    Chat, ChatEvent, Subscription, WatchTarget,
    watch::{EndReason, NamedTarget, OpenTask, OpenWork, WatchCommand, WatchLine, WatchSummary},
  },
  store::TaskStore,
  task::organization::ChangeActor,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{
  harness::{Orchestrator, Setup},
  standing::waiting,
};

/// Module *Webhooks v2*, holding task `task`, as a target named as people read it.
fn webhooks(db: &mut ProjectDb, task: orchester_core::id::TaskId) -> NamedTarget {
  let name = Name::new("Webhooks v2").expect("name");
  let details = ModuleDetails::new(name, "", (None, None), ModuleState::Open).expect("details");
  let module = db.create_module(details, Timestamp::now()).expect("module");
  db.set_task_module(task, Some(module.id()), ChangeActor::Person, Timestamp::now())
    .expect("moved");
  NamedTarget {
    target: WatchTarget::Module(module.id()),
    name: Some("Webhooks v2".to_owned()),
  }
}

/// Ends a subscription of `chat` to a module named *Mobile release* by itself, every task Done, with no turn.
fn ended(db: &mut ProjectDb, chat: &Chat) {
  let subscription = Subscription::new(
    chat.id(),
    WatchTarget::Module(ModuleId::generate()),
    "",
    Timestamp::now(),
  );
  db.start_subscription(&subscription).expect("started");
  let summary = WatchSummary {
    target: subscription.target(),
    name: "Mobile release".to_owned(),
    title: None,
    reason: EndReason::ModuleClosed { done: 3, canceled: 0 },
    order: None,
    since: subscription.since(),
    link: "/modules".to_owned(),
    automatic: false,
  };
  assert!(
    db.end_subscription_by_itself(&subscription, &summary, Timestamp::now())
      .expect("ended")
  );
}

/// The id of the row of watch entry `seq`.
fn row(seq: i64) -> String {
  format!("orchestrator-watch-wait-{seq}")
}

/// A conversation *Release checklist* holding the person's `/watch-task` of `task`, with no order.
fn checklist(db: &mut ProjectDb, task: &NamedTarget) -> Chat {
  let checklist = Chat::named(ChatId::generate(), "Release checklist", Timestamp::now());
  let command = WatchCommand::Watch {
    targets: vec![task.clone()],
    order: String::new(),
  };
  db.keep_watch_entries(&checklist, &[ChatEvent::WatchCommand { command }], Timestamp::now())
    .expect("kept");
  checklist
}

/// The entries of the conversation under test: a `/watch-module` of `module` with its subscription line, `task` (key
/// `key`) taken by `checklist`, and a stop of `module` that left that task queued.
fn entries(module: &NamedTarget, (task, key): (NamedTarget, &str), checklist: &Chat) -> [ChatEvent; 4] {
  let line = |line| ChatEvent::Watch { line };
  [
    ChatEvent::WatchCommand {
      command: WatchCommand::Watch {
        targets: vec![module.clone()],
        order: "Answer agents from the brief.".to_owned(),
      },
    },
    line(WatchLine::Watching {
      started: vec![module.clone()],
      already_here: Vec::new(),
      led_elsewhere: Vec::new(),
      gone: Vec::new(),
    }),
    line(WatchLine::NowLedIn {
      target: task,
      chat: checklist.id(),
      title: checklist.title().to_owned(),
      taken_over: false,
    }),
    line(WatchLine::Stopped {
      targets: vec![module.clone()],
      left_open: vec![OpenTask {
        task: key.to_owned(),
        work: OpenWork::Queued,
      }],
    }),
  ]
}

/// The command's bubble `command` and the subscription line `leading`.
fn assert_command(screen: &mut Orchestrator, (command, leading): (&str, &str)) {
  assert_eq!(screen.text(&format!("{command}-command-name")), "/watch-module");
  assert_eq!(screen.text(&format!("{command}-target-0-name")), "Webhooks v2");
  assert_eq!(
    screen.text(&format!("{command}-order")),
    "Answer agents from the brief."
  );
  assert!(
    screen.text(&format!("{command}-delivery")).starts_with("Applied "),
    "the command says it was applied, and when"
  );
  assert_eq!(
    screen.text(&format!("{leading}-aside")),
    "— their waits, run ends, conflicts, limits and stalls start a turn here"
  );
}

/// The lines `led_in`, `stopped` and `summary` about task `key`.
fn assert_lines(screen: &mut Orchestrator, (led_in, stopped, summary): (&str, &str, &str), key: &str) {
  assert_eq!(
    screen.text(&format!("{led_in}-words")),
    format!("{key} is now led in «Release checklist»")
  );
  assert_eq!(
    screen.text(&format!("{led_in}-aside")),
    "— watched there as a task; the rest of Webhooks v2 stays here"
  );
  assert_eq!(screen.text(&format!("{led_in}-open")), "Open ↗");
  assert_eq!(
    screen.text(&format!("{stopped}-line-words")),
    "Stopped watching Webhooks v2"
  );
  assert_eq!(
    screen.text(&format!("{stopped}-line-aside")),
    "— 1 left open goes back to the usual rule"
  );
  assert_eq!(screen.text(&format!("{stopped}-left-0")), format!("{key} queued ↗"));
  assert_eq!(
    screen.text(&format!("{summary}-words")),
    "Every task of Mobile release is Done"
  );
  assert_eq!(screen.text(&format!("{summary}-aside")), "— watching ended");
}

#[test]
fn orchestrator_watch_entries_are_rows_of_their_own_as_the_lines_design_shows() {
  let mut screen = Orchestrator::new(Setup::NoRunner);
  let delivery = screen.delivery();
  let (run, _) = waiting(&screen, delivery, "Retry failed deliveries");
  let mut db = screen.db();
  let key = db.task(run.task()).expect("read").expect("the task").key().to_string();
  let module = webhooks(&mut db, run.task());
  let task = NamedTarget {
    target: WatchTarget::Task(run.task()),
    name: Some(key.clone()),
  };
  let checklist = checklist(&mut db, &task);
  let chat = Chat::named(ChatId::generate(), "Webhooks v2", Timestamp::now());
  db.keep_watch_entries(&chat, &entries(&module, (task, &key), &checklist), Timestamp::now())
    .expect("kept");
  ended(&mut db, &chat);
  let seqs: Vec<i64> = db
    .chat_entries(chat.id())
    .expect("entries")
    .into_iter()
    .map(|entry| entry.seq)
    .collect();
  let checklist_command = db.chat_entries(checklist.id()).expect("entries")[0].seq;
  drop(db);
  let &[command, leading, led_in, stopped, summary] = seqs.as_slice() else {
    panic!("five entries: {seqs:?}");
  };
  let leading = row(leading);

  screen.go(OrchestratorChat::Chat(chat.id()));
  screen.until("the subscription line counts the module's task", |screen| {
    screen.text(&format!("{leading}-words")) == "Leading Webhooks v2 · 1 task"
  });
  assert_command(&mut screen, (&row(command), &leading));
  let summary = format!("orchestrator-watch-ended-wait-{summary}");
  assert_lines(&mut screen, (&row(led_in), &row(stopped), &summary), &key);

  screen.go(OrchestratorChat::Chat(checklist.id()));
  let checklist_command = row(checklist_command);
  screen.until("the /watch-task bubble", |screen| {
    screen.exists(&format!("{checklist_command}-bubble"))
  });
  assert_eq!(screen.text(&format!("{checklist_command}-command-name")), "/watch-task");
  assert!(
    !screen.exists(&format!("{checklist_command}-order")),
    "a command without an order shows its chips alone"
  );
}
