//! `/unwatch` (ORC-774, `Parts · /unwatch` `NtsXc`; its picker v2, ORC-889, as `P1` `QXPDY`; the badge ORC-890, P5
//! `ijspu`): *Stop watching* opens its picker while the conversation leads something. The picker has its own search
//! field and lists only what this conversation watches (*Watched here · 3*), each with its kind, since when and what
//! is still open there, then *All of them*, and the keys. Enter adds the focused row to the *Stop watching* badge and
//! closes the picker; then Enter stops the badge's subscriptions and keeps the person's `/unwatch`, a message written
//! with it optional; *All of them* stops everything; automation's Enter stops nothing.
//! `cargo test -p orchester-app --test orchestrator unwatch`.

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::OrchestratorChat;
use orchester_core::{
  id::{ChatId, IdempotencyKey, ModuleId, TaskId},
  module::{ModuleDetails, ModuleState},
  name::Name,
  orchestrator::{
    Chat, ChatEvent, EntryKind, WatchTarget,
    watch::{NamedTarget, WatchCommand},
  },
  store::TaskStore,
  task::{TaskDetails, TaskLimits, organization::ChangeActor},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::orchestrator::watch::watch_in;

use super::{
  harness::{Orchestrator, Setup},
  history::kept,
  watch_module::badge_words,
};

const INPUT: &str = "orchestrator-input";
const PICKER: &str = "orchestrator-composer-unwatch-picker";
const BADGE: &str = "orchestrator-composer-badge-unwatch";
const SEARCH: &str = "orchestrator-composer-unwatch-search";
const COMMAND: &str = "orchestrator-composer-slash-unwatch";

/// A task titled `title`, with no pipeline.
fn task(db: &mut ProjectDb, title: &str) -> TaskId {
  let details = TaskDetails {
    title: Name::new(title).expect("a title"),
    brief: "".into(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::new(None, None).expect("no limits"),
  };
  db.create_tasks(IdempotencyKey::generate(), vec![details], Timestamp::now())
    .expect("a task")[0]
    .id()
}

fn module(db: &mut ProjectDb, name: &str) -> ModuleId {
  let details =
    ModuleDetails::new(Name::new(name).expect("a name"), "", (None, None), ModuleState::Open).expect("details");
  db.create_module(details, Timestamp::now()).expect("a module").id()
}

/// The conversation *Webhooks work* watches Webhooks v2 (one open task with nothing under way), Search index (no task)
/// and a task of its own; *Billing and invoices* watches the tasks without a module. The screen shows *Webhooks work*.
struct Watched {
  screen: Orchestrator,
  here: ChatId,
  other: ChatId,
  webhooks: ModuleId,
  search: ModuleId,
  own: TaskId,
}

fn watched() -> Watched {
  let mut screen = Orchestrator::new(Setup::Ready);
  let other = kept(&screen, "Billing and invoices", Timestamp::now(), "Billing it is.");
  let here = kept(&screen, "Webhooks work", Timestamp::now(), "Noted.");
  let mut db = screen.db();
  let webhooks = module(&mut db, "Webhooks v2");
  let retry = task(&mut db, "Retry failed deliveries");
  db.set_task_module(retry, Some(webhooks), ChangeActor::Person, Timestamp::now())
    .expect("in the module");
  let search = module(&mut db, "Search index");
  let own = task(&mut db, "Shared rate limiter");
  drop(db);
  let named = |target, name: Option<&str>| NamedTarget {
    target,
    name: name.map(str::to_owned),
  };
  let watch = |chat, targets: &[NamedTarget]| {
    watch_in(&screen.root, &Chat::named(chat, "", Timestamp::now()), targets, "").expect("watched");
  };
  watch(
    here,
    &[
      named(WatchTarget::Module(webhooks), Some("Webhooks v2")),
      named(WatchTarget::Module(search), Some("Search index")),
      named(WatchTarget::Task(own), None),
    ],
  );
  watch(other, &[named(WatchTarget::WithoutModule, None)]);
  screen.go(OrchestratorChat::Chat(here));
  screen.until("the conversation", |screen| {
    screen.text("orchestrator-conversation").contains("Noted.")
  });
  Watched {
    screen,
    here,
    other,
    webhooks,
    search,
    own,
  }
}

/// *Stop watching* chosen in the slash menu with `/unwatch`, its own name, where it takes the focus: the command's
/// badge and its picker.
fn open_picker(screen: &mut Orchestrator) {
  screen
    .window
    .tree
    .get_element_by_id_mut(INPUT)
    .expect("the message field")
    .focus();
  screen.set(INPUT, "/unwatch");
  screen.until("Stop watching to run", |screen| {
    screen.exists(COMMAND) && screen.text(COMMAND).contains("Enter")
  });
  screen.key(("Enter", "Enter"), false);
  screen.until("the picker", |screen| screen.exists(PICKER) && screen.exists(BADGE));
}

/// The focused row added to the badge with Enter, which closes the picker.
fn pick(screen: &mut Orchestrator) {
  screen.key(("Enter", "Enter"), false);
  screen.until("the picker closed", |screen| !screen.exists(PICKER));
}

fn row(key: impl std::fmt::Display) -> String {
  format!("orchestrator-composer-unwatch-{key}")
}

/// The targets `chat` leads now.
fn led(screen: &Orchestrator, chat: ChatId) -> Vec<WatchTarget> {
  screen
    .db()
    .active_subscriptions()
    .expect("subscriptions")
    .of_chat(chat)
    .map(|subscription| subscription.target())
    .collect()
}

#[test]
fn orchestrator_unwatch_picker_lists_only_what_this_conversation_watches_with_its_open_work() {
  let Watched {
    mut screen,
    webhooks,
    search,
    own,
    ..
  } = watched();
  open_picker(&mut screen);
  screen.until("the subscriptions", |screen| {
    screen.exists(&row(format!("module-{webhooks}")))
  });
  assert!(screen.exists(SEARCH), "the picker's own search field");
  let shown = screen.text(PICKER).replace('\u{a0}', " ");
  assert!(shown.contains("Watched here 3"), "three subscriptions: {shown}");
  for words in [
    "Webhooks v2",
    "module · since",
    "nothing under way",
    "Search index",
    "nothing open",
    "task · since",
    "All of them",
    "stop watching everything here",
    "cancels nothing",
    "Move",
    "Pick and close",
    "Shift Enter",
    "Add another",
    "Close",
  ] {
    assert!(shown.contains(words), "{words}: {shown}");
  }
  assert!(!shown.contains("Ctrl Enter"), "no Ctrl Enter: {shown}");
  assert!(
    !shown.contains("Tasks without a module"),
    "another conversation's subscription is not listed: {shown}"
  );
  let order: Vec<f32> = [
    row(format!("module-{webhooks}")),
    row(format!("module-{search}")),
    row(format!("task-{own}")),
    row("all"),
  ]
  .iter()
  .map(|id| screen.bounds(id).y)
  .collect();
  assert!(
    order.windows(2).all(|pair| pair[0] < pair[1]),
    "in NtsXc's order: {order:?}"
  );
  assert!(
    (screen.bounds(PICKER).width - 560.0).abs() < 0.5,
    "560 wide: {:?}",
    screen.bounds(PICKER)
  );

  screen.set(SEARCH, "sea");
  screen.until("only Search index", |screen| {
    screen.exists(&row(format!("module-{search}")))
      && !screen.exists(&row(format!("module-{webhooks}")))
      && screen.text(PICKER).contains("1 of 3")
  });
  assert!(screen.exists(&row("all")), "All of them stays");
}

#[test]
fn orchestrator_unwatch_enter_stops_the_chosen_and_all_of_them_stops_the_rest() {
  let Watched {
    mut screen,
    here,
    other,
    webhooks,
    search,
    own,
  } = watched();
  open_picker(&mut screen);
  screen.until("the subscriptions", |screen| {
    screen.exists(&row(format!("module-{webhooks}")))
  });
  pick(&mut screen);
  assert_eq!(badge_words(&mut screen, BADGE), "Stop watching · Webhooks v2");
  assert_eq!(screen.text("orchestrator-keys"), "Enter", "Enter sends now");
  screen.key(("Enter", "Enter"), false);
  screen.until("Webhooks v2 to stop", |screen| {
    !led(screen, here).contains(&WatchTarget::Module(webhooks))
  });
  let mut left = led(&screen, here);
  left.sort_by_key(|target| format!("{target:?}"));
  let mut expected = vec![WatchTarget::Module(search), WatchTarget::Task(own)];
  expected.sort_by_key(|target| format!("{target:?}"));
  assert_eq!(left, expected, "only the chosen one stops");
  screen.until("the badge to go", |screen| !screen.exists(BADGE));
  let kept: Vec<EntryKind> = screen
    .db()
    .chat_entries(here)
    .expect("entries")
    .into_iter()
    .map(|entry| entry.kind)
    .collect();
  assert!(
    kept.iter().any(|kind| matches!(
      kind,
      EntryKind::Event(ChatEvent::WatchCommand {
        command: WatchCommand::Unwatch { targets },
      }) if targets.iter().map(|named| named.target).eq([WatchTarget::Module(webhooks)])
    )),
    "the person's /unwatch is kept: {kept:?}"
  );
  assert_eq!(
    kept
      .iter()
      .filter(|kind| matches!(kind, EntryKind::Person { .. }))
      .count(),
    1,
    "with nothing written, no message goes: {kept:?}"
  );

  open_picker(&mut screen);
  screen.until("the two left", |screen| {
    screen.text(PICKER).replace('\u{a0}', " ").contains("Watched here 2")
  });
  // Search index, the task, then All of them; Enter picks it and closes the picker.
  screen.key(("ArrowDown", "ArrowDown"), false);
  screen.key(("ArrowDown", "ArrowDown"), false);
  screen.key(("Enter", "Enter"), false);
  screen.until("All of them in the badge, the picker closed", |screen| {
    !screen.exists(PICKER) && badge_words(screen, BADGE) == "Stop watching · All of them"
  });
  screen.key(("Enter", "Enter"), false);
  screen.until("everything here to stop", |screen| led(screen, here).is_empty());
  assert_eq!(
    led(&screen, other),
    vec![WatchTarget::WithoutModule],
    "another conversation's subscription goes on"
  );
}

#[test]
fn orchestrator_unwatch_refuses_synthetic_enter_and_keeps_the_badge() {
  let Watched {
    mut screen,
    here,
    webhooks,
    ..
  } = watched();
  open_picker(&mut screen);
  screen.until("the subscriptions", |screen| {
    screen.exists(&row(format!("module-{webhooks}")))
  });
  pick(&mut screen);

  with_synthetic_input(|| screen.key(("Enter", "Enter"), false));
  let problem = "orchestrator-watching-problem";
  screen.until("the person-only notice", |screen| {
    screen.exists(problem) && screen.text(problem).contains("Only a person stops watching")
  });
  assert_eq!(led(&screen, here).len(), 3, "synthetic input stops nothing");
  assert_eq!(
    badge_words(&mut screen, BADGE),
    "Stop watching · Webhooks v2",
    "the badge stays"
  );
}
