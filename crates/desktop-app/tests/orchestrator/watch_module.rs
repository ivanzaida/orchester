//! `/watch-module` (ORC-772, `02 · Choosing modules` `c8pALl` in `Orchestrator watch · redesign`; the badge ORC-890,
//! P2 `WbHSt` in `Proposal · watch composer v2` `LRg3R`): its picker lists the project's open modules with what they
//! take in, *Led in «…» ↗* for one another conversation leads (which opens it), a module with nothing left to watch
//! faded, then *Also* with the tasks without a module and everything in the project, and the keys; Enter adds the
//! focused module to the command's one badge, and the row then reads *Added*; the words typed filter the modules by
//! name while the picker is open. Once it is closed the composer is the normal chat input: Enter sends the message,
//! and the conversation watches the badge's modules with the message as their order.

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::{OrchestratorChat, ProjectPage, Route};
use orchester_core::{
  id::{ChatId, IdempotencyKey, ModuleId, TaskId},
  module::{ModuleDetails, ModuleState},
  name::Name,
  orchestrator::{EntryKind, Subscription, WatchTarget},
  store::TaskStore,
  task::{
    TaskDetails, TaskLimits,
    organization::{ChangeActor, WorkflowStatus},
  },
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{
  harness::{Orchestrator, Setup},
  history::kept,
};

const INPUT: &str = "orchestrator-input";
const PICKER: &str = "orchestrator-composer-watch-module-picker";
pub(super) const BADGE: &str = "orchestrator-composer-badge-watch-module";

/// The person's messages kept in conversation `chat`, in order.
pub(super) fn messages(screen: &Orchestrator, chat: ChatId) -> Vec<String> {
  screen
    .db()
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Person { text, .. } => Some(String::from(text)),
      _ => None,
    })
    .collect()
}

/// What the composer's badge `badge` says, without its icons: its verb, then `·` and its choices, then *+N*.
pub(super) fn badge_words(screen: &mut Orchestrator, badge: &str) -> String {
  let mut words = screen.text(&format!("{badge}-verb"));
  let targets = screen.text(&format!("{badge}-targets"));
  if !targets.is_empty() {
    words = format!("{words} · {targets}");
  }
  let more = screen.text(&format!("{badge}-more"));
  if !more.is_empty() {
    words = format!("{words} {more}");
  }
  words
}

/// A task titled `title`, with no pipeline.
pub(super) fn task(db: &mut ProjectDb, title: &str) -> TaskId {
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

/// Module `name` holding the tasks titled `titles`.
pub(super) fn module(db: &mut ProjectDb, name: &str, titles: &[&str]) -> (ModuleId, Vec<TaskId>) {
  let details =
    ModuleDetails::new(Name::new(name).expect("a name"), "", (None, None), ModuleState::Open).expect("details");
  let module = db.create_module(details, Timestamp::now()).expect("a module").id();
  let tasks = titles
    .iter()
    .map(|title| {
      let id = task(db, title);
      db.set_task_module(id, Some(module), ChangeActor::Person, Timestamp::now())
        .expect("in the module");
      id
    })
    .collect();
  (module, tasks)
}

/// The person's first message of the conversation shown, and its title.
const TITLE: &str = "Split Webhooks v2 and Billing";

/// Kontur's modules: Webhooks v2 (two tasks), Billing (one, led in *Billing and invoices*) and Mobile release (one,
/// Done); one task without a module. The screen shows the conversation *Split Webhooks v2 and Billing*.
struct Kontur {
  screen: Orchestrator,
  here: ChatId,
  billing_chat: ChatId,
  webhooks: ModuleId,
  billing: ModuleId,
  limiter: TaskId,
}

fn kontur() -> Kontur {
  let mut screen = Orchestrator::new(Setup::Ready);
  let billing_chat = kept(&screen, "Billing and invoices", Timestamp::now(), "Billing it is.");
  let here = kept(&screen, TITLE, Timestamp::now(), "Tell me what to watch.");
  let mut db = screen.db();
  let (webhooks, _) = module(
    &mut db,
    "Webhooks v2",
    &["Retry failed deliveries", "Sign webhook payloads"],
  );
  let (billing, _) = module(&mut db, "Billing", &["Invoice export"]);
  let (_, mobile) = module(&mut db, "Mobile release", &["Store screenshots"]);
  db.close_task_organization(mobile[0], WorkflowStatus::Done, (ChangeActor::Person, Timestamp::now()))
    .expect("done");
  let limiter = task(&mut db, "Shared rate limiter");
  let led = Subscription::new(billing_chat, WatchTarget::Module(billing), "", Timestamp::now());
  db.start_subscription(&led).expect("led");
  drop(db);
  screen.go(OrchestratorChat::Chat(here));
  screen.until("the conversation", |screen| {
    screen
      .text("orchestrator-conversation")
      .contains("Tell me what to watch.")
  });
  Kontur {
    screen,
    here,
    billing_chat,
    webhooks,
    billing,
    limiter,
  }
}

/// The focus in the message field.
pub(super) fn focus_field(screen: &mut Orchestrator) {
  screen
    .window
    .tree
    .get_element_by_id_mut(INPUT)
    .expect("the message field")
    .focus();
}

/// *Watch modules* chosen in the slash menu, where it is the first command `/watch` matches that can run: the
/// command's badge and its picker.
fn open_picker(screen: &mut Orchestrator) {
  focus_field(screen);
  screen.set(INPUT, "/watch");
  let command = "orchestrator-composer-slash-watch-module";
  screen.until("Watch modules", |screen| {
    screen.exists(command) && screen.text(command).contains("Their tasks' events come here")
  });
  assert!(screen.text(command).contains("Enter"), "it takes the focus");
  screen.key(("Enter", "Enter"), false);
  screen.until("the picker", |screen| screen.exists(PICKER) && screen.exists(BADGE));
}

fn row(id: impl std::fmt::Display) -> String {
  format!("orchestrator-composer-watch-module-{id}")
}

/// Whether `module` is watched by a conversation now.
fn watched(screen: &Orchestrator, module: ModuleId) -> bool {
  screen
    .db()
    .active_subscriptions()
    .expect("read")
    .of(WatchTarget::Module(module))
    .is_some()
}

/// Webhooks v2 added to the badge with Enter in the open picker, and the picker closed with Esc.
fn webhooks_chosen(screen: &mut Orchestrator, webhooks: ModuleId) {
  open_picker(screen);
  screen.until("the modules", |screen| screen.exists(&row(webhooks)));
  screen.key(("Enter", "Enter"), false);
  screen.until("Webhooks v2 in the badge", |screen| {
    badge_words(screen, BADGE) == "Watch · Webhooks v2"
  });
  screen.key(("Escape", "Escape"), false);
  screen.until("the picker closed", |screen| !screen.exists(PICKER));
}

#[test]
fn orchestrator_watch_module_picker_lists_modules_who_leads_them_and_the_whole_targets() {
  let Kontur {
    mut screen,
    webhooks,
    billing,
    billing_chat,
    ..
  } = kontur();
  open_picker(&mut screen);
  screen.until("the modules", |screen| screen.exists(&row(webhooks)));
  let shown = screen.text(PICKER).replace('\u{a0}', " ");
  assert!(shown.starts_with("Modules 3"), "three open modules: {shown}");
  for words in [
    "Webhooks v2",
    "2 tasks · none running",
    "Billing",
    "Led in «Billing and invoices» ↗",
    "Mobile release",
    "1 task · all done",
    "Nothing left to watch",
    "Also",
    "Tasks without a module",
    "Everything in",
    "What no other subscription takes",
    "Move",
    "Add",
    "Remove the last",
    "Close",
  ] {
    assert!(shown.contains(words), "{words}: {shown}");
  }
  assert!(!shown.contains("Ctrl Enter"), "no Ctrl Enter: {shown}");
  let webhooks_row = screen.text(&row(webhooks));
  assert!(
    webhooks_row.contains("Enter"),
    "the focus starts on the first module: {webhooks_row}"
  );
  let order: Vec<f32> = ["Webhooks v2", "Billing", "Mobile release", "Tasks without a module"]
    .iter()
    .map(|words| {
      let found = screen.text(PICKER).replace('\u{a0}', " ").find(words).expect("shown");
      found as f32
    })
    .collect();
  assert!(
    order.windows(2).all(|pair| pair[0] < pair[1]),
    "in 02's order: {order:?}"
  );
  assert!(
    (screen.bounds(PICKER).width - 560.0).abs() < 0.5,
    "560 wide: {:?}",
    screen.bounds(PICKER)
  );

  screen.key(("Enter", "Enter"), false);
  screen.until("Webhooks v2 added", |screen| {
    screen.text(&row(webhooks)).contains("Added")
  });
  assert_eq!(
    badge_words(&mut screen, BADGE),
    "Watch · Webhooks v2",
    "the module joins the badge"
  );

  screen.click(&row(format!("{billing}-led-in")));
  screen.until("the leading conversation", |screen| {
    screen.route()
      == Route::Project(
        screen.project,
        ProjectPage::Orchestrator(OrchestratorChat::Chat(billing_chat)),
      )
  });
}

#[test]
fn orchestrator_watch_module_one_badge_names_two_modules_and_counts_the_rest() {
  let Kontur {
    mut screen, webhooks, ..
  } = kontur();
  assert!(
    !screen.exists("orchestrator-composer-badges"),
    "no badge row while nothing is picked"
  );
  open_picker(&mut screen);
  assert_eq!(
    badge_words(&mut screen, BADGE),
    "Watch",
    "the verb alone while nothing is chosen"
  );
  screen.until("the modules", |screen| screen.exists(&row(webhooks)));
  // Webhooks v2, then Tasks without a module and Everything in Kontur: Billing is led elsewhere and Mobile release has
  // nothing left, so the focus steps over them as the rows can be chosen.
  for _ in 0..3 {
    screen.key(("Enter", "Enter"), false);
    screen.key(("ArrowDown", "ArrowDown"), false);
  }
  screen.until("three in the badge", |screen| screen.exists(&format!("{BADGE}-more")));
  let words = badge_words(&mut screen, BADGE);
  assert!(
    words.starts_with("Watch · Webhooks v2, ") && words.ends_with(" +1"),
    "{words}"
  );
  assert_eq!(
    screen.text("orchestrator-composer-badges").matches("Watch").count(),
    1,
    "one badge for the command"
  );
  let (badge, field) = (screen.bounds(BADGE), screen.bounds(INPUT));
  assert!(
    badge.y + badge.height <= field.y,
    "pinned over the message: {badge:?} {field:?}"
  );
}

#[test]
fn orchestrator_watch_module_typed_after_picking_is_a_message_and_enter_sends_it_with_the_watch() {
  let Kontur {
    mut screen,
    here,
    webhooks,
    billing,
    ..
  } = kontur();
  open_picker(&mut screen);
  screen.set(INPUT, "hoo");
  // The matched letters are a piece of their own, so the row is found by its id rather than its whole name.
  screen.until("only Webhooks v2", |screen| {
    screen.exists(&row(webhooks)) && !screen.exists(&row(billing)) && screen.text(PICKER).contains("1 of 3")
  });
  assert!(screen.text(&row(webhooks)).contains("hoo"), "the matched letters show");
  screen.key(("Enter", "Enter"), false);
  screen.until("the badge", |screen| {
    badge_words(screen, BADGE) == "Watch · Webhooks v2"
  });
  screen.key(("Escape", "Escape"), false);
  screen.until("the picker closed", |screen| !screen.exists(PICKER));
  assert!(
    screen.text(INPUT).contains("Message the Orchestrator…"),
    "the normal placeholder: {}",
    screen.text(INPUT)
  );
  assert_eq!(screen.text("orchestrator-keys"), "Enter", "Enter sends now");

  screen.set(INPUT, "Ask before merging.");
  assert!(!screen.exists(PICKER), "the words are a message, not a filter");
  screen.key(("Enter", "Enter"), false);
  screen.until("the subscription", |screen| watched(screen, webhooks));
  let subscriptions = screen.db().active_subscriptions().expect("read");
  let led = subscriptions.of(WatchTarget::Module(webhooks)).expect("led");
  assert_eq!(led.chat(), here, "this conversation leads it");
  assert_eq!(led.order(), Some("Ask before merging."), "the message is the order");
  screen.until("the message sent", |screen| {
    messages(screen, here).iter().any(|text| text == "Ask before merging.")
  });
  screen.until("the answer", |screen| {
    screen
      .text("orchestrator-conversation")
      .contains("Noted: Ask before merging.")
  });
  screen.until("the badge to go", |screen| !screen.exists(BADGE));
}

#[test]
fn orchestrator_watch_module_enter_with_nothing_written_watches_with_the_default_order() {
  let Kontur {
    mut screen,
    here,
    webhooks,
    ..
  } = kontur();
  webhooks_chosen(&mut screen, webhooks);
  screen.key(("Enter", "Enter"), false);
  screen.until("the subscription", |screen| watched(screen, webhooks));
  let subscriptions = screen.db().active_subscriptions().expect("read");
  assert_eq!(
    subscriptions.of(WatchTarget::Module(webhooks)).expect("led").order(),
    None,
    "the project's default order"
  );
  screen.until("the badge to go", |screen| !screen.exists(BADGE));
  assert_eq!(messages(&screen, here), [TITLE], "no message of its own");
}

#[test]
fn orchestrator_watch_module_x_and_backspace_remove_the_badge() {
  let Kontur {
    mut screen, webhooks, ..
  } = kontur();
  webhooks_chosen(&mut screen, webhooks);
  screen.click(&format!("{BADGE}-remove"));
  screen.until("the badge removed", |screen| !screen.exists(BADGE));
  assert!(!screen.exists("orchestrator-composer-badges"), "and its row");

  webhooks_chosen(&mut screen, webhooks);
  focus_field(&mut screen);
  screen.key(("Backspace", "Backspace"), false);
  screen.until("the badge removed", |screen| !screen.exists(BADGE));
  assert!(!watched(&screen, webhooks), "nothing was watched");
}

#[test]
fn orchestrator_watch_module_refuses_synthetic_enter_and_keeps_the_badge_and_the_message() {
  let Kontur {
    mut screen,
    here,
    webhooks,
    ..
  } = kontur();
  webhooks_chosen(&mut screen, webhooks);
  screen.set(INPUT, "Ask before merging.");

  with_synthetic_input(|| screen.key(("Enter", "Enter"), false));
  let problem = "orchestrator-watch-problem";
  screen.until("the person-only notice", |screen| {
    screen.exists(problem) && screen.text(problem).contains("Only you can start watching")
  });
  assert!(!watched(&screen, webhooks), "synthetic input watches nothing");
  assert!(screen.exists(BADGE), "the badge stays");
  assert!(screen.text(INPUT).contains("Ask before merging."), "and the message");
  assert_eq!(messages(&screen, here), [TITLE], "nothing was sent");

  // The person's own Enter, back in the message field, applies what stayed, the message included.
  focus_field(&mut screen);
  screen.key(("Enter", "Enter"), false);
  screen.until("the subscription", |screen| watched(screen, webhooks));
  let subscriptions = screen.db().active_subscriptions().expect("read");
  let led = subscriptions.of(WatchTarget::Module(webhooks)).expect("led");
  assert_eq!(led.chat(), here, "this conversation leads it");
  assert_eq!(led.order(), Some("Ask before merging."), "the order stayed");
}

#[test]
fn orchestrator_watch_module_and_task_badges_in_one_message_both_apply_on_enter() {
  let Kontur {
    mut screen,
    here,
    webhooks,
    limiter,
    ..
  } = kontur();
  webhooks_chosen(&mut screen, webhooks);
  focus_field(&mut screen);
  screen.set(INPUT, "/watch-task");
  let command = "orchestrator-composer-slash-watch-task";
  screen.until("Watch tasks focused", |screen| {
    screen.exists(command) && screen.text(command).contains("Enter")
  });
  screen.key(("Enter", "Enter"), false);
  let (task_picker, task_badge) = (
    "orchestrator-composer-watch-task-picker",
    "orchestrator-composer-badge-watch-task",
  );
  screen.until("the task picker", |screen| {
    screen.exists(task_picker) && screen.exists(task_badge)
  });
  screen.set(INPUT, "rate lim");
  let limiter_key = screen
    .db()
    .task(limiter)
    .expect("read")
    .expect("the task")
    .key()
    .to_string();
  let limiter_row = format!("orchestrator-composer-watch-task-{limiter_key}");
  screen.until("only the limiter", |screen| screen.exists(&limiter_row));
  screen.key(("Enter", "Enter"), false);
  let words = format!("Watch · {limiter_key}");
  screen.until("the task badge", |screen| badge_words(screen, task_badge) == words);
  screen.key(("Escape", "Escape"), false);
  screen.until("the picker closed", |screen| !screen.exists(task_picker));
  assert_eq!(
    screen.text("orchestrator-composer-badges").matches("Watch").count(),
    2,
    "two badges, one per command"
  );

  screen.set(INPUT, "Ask before merging.");
  screen.key(("Enter", "Enter"), false);
  let both = |screen: &mut Orchestrator| {
    let subscriptions = screen.db().active_subscriptions().expect("read");
    subscriptions.of(WatchTarget::Module(webhooks)).is_some() && subscriptions.of(WatchTarget::Task(limiter)).is_some()
  };
  screen.until("both subscriptions", both);
  let subscriptions = screen.db().active_subscriptions().expect("read");
  for target in [WatchTarget::Module(webhooks), WatchTarget::Task(limiter)] {
    let led = subscriptions.of(target).expect("led");
    assert_eq!(led.chat(), here, "this conversation leads {target:?}");
    assert_eq!(led.order(), Some("Ask before merging."), "the message is the order");
  }
  screen.until("the message sent", |screen| {
    messages(screen, here).iter().any(|text| text == "Ask before merging.")
  });
  screen.until("the badges to go", |screen| {
    !screen.exists(BADGE) && !screen.exists(task_badge)
  });
  assert!(!screen.exists("orchestrator-watch-problem"), "no watch failed");
}

#[test]
fn orchestrator_watch_module_applied_offers_unwatch_in_the_slash_menu() {
  let Kontur {
    mut screen, webhooks, ..
  } = kontur();
  webhooks_chosen(&mut screen, webhooks);
  screen.key(("Enter", "Enter"), false);
  screen.until("the subscription", |screen| watched(screen, webhooks));
  screen.until("the badge to go", |screen| !screen.exists(BADGE));
  focus_field(&mut screen);
  screen.set(INPUT, "/unwatch");
  let command = "orchestrator-composer-slash-unwatch";
  screen.until("Stop watching offered", |screen| {
    screen.exists(command) && screen.text(command).contains("nothing is canceled")
  });
  assert!(
    !screen
      .text("orchestrator-composer-slash-menu")
      .contains("No command or skill matches"),
    "{}",
    screen.text("orchestrator-composer-slash-menu")
  );
}
