//! `/watch-module` (ORC-772, `02 · Choosing modules` `c8pALl` in `Orchestrator watch · redesign`): its picker lists the
//! project's open modules with what they take in, *Led in «…» ↗* for one another conversation leads (which opens it),
//! a module with nothing left to watch faded, then *Also* with the tasks without a module and everything in the
//! project, and the keys; Enter makes the focused module a chip, which then reads *Added*; the words typed filter the
//! modules by name; Ctrl Enter has the conversation watch the chips with the words after them as their order.

use orchester_app::routes::{OrchestratorChat, ProjectPage, Route};
use orchester_core::{
  id::{ChatId, IdempotencyKey, ModuleId, TaskId},
  module::{ModuleDetails, ModuleState},
  name::Name,
  orchestrator::{Subscription, WatchTarget},
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
const CHIP: &str = "orchestrator-composer-command-chip";

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

/// Module `name` holding the tasks titled `titles`.
fn module(db: &mut ProjectDb, name: &str, titles: &[&str]) -> (ModuleId, Vec<TaskId>) {
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

/// Kontur's modules: Webhooks v2 (two tasks), Billing (one, led in *Billing and invoices*) and Mobile release (one,
/// Done); one task without a module. The screen shows the conversation *Split Webhooks v2 and Billing*.
struct Kontur {
  screen: Orchestrator,
  here: ChatId,
  billing_chat: ChatId,
  webhooks: ModuleId,
  billing: ModuleId,
}

fn kontur() -> Kontur {
  let mut screen = Orchestrator::new(Setup::Ready);
  let billing_chat = kept(&screen, "Billing and invoices", Timestamp::now(), "Billing it is.");
  let here = kept(
    &screen,
    "Split Webhooks v2 and Billing",
    Timestamp::now(),
    "Tell me what to watch.",
  );
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
  task(&mut db, "Shared rate limiter");
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
  }
}

/// *Watch modules* chosen in the slash menu, where it is the first command `/watch` matches that can run: the command
/// chip and its picker.
fn open_picker(screen: &mut Orchestrator) {
  screen
    .window
    .tree
    .get_element_by_id_mut(INPUT)
    .expect("the message field")
    .focus();
  screen.set(INPUT, "/watch");
  let command = "orchestrator-composer-slash-watch-module";
  screen.until("Watch modules", |screen| {
    screen.exists(command) && screen.text(command).contains("Their tasks' events come here")
  });
  assert!(screen.text(command).contains("Enter"), "it takes the focus");
  screen.key(("Enter", "Enter"), false);
  screen.until("the picker", |screen| screen.exists(PICKER) && screen.exists(CHIP));
}

fn row(id: impl std::fmt::Display) -> String {
  format!("orchestrator-composer-watch-module-{id}")
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
    "Ctrl Enter",
    "Watch",
    "Close",
  ] {
    assert!(shown.contains(words), "{words}: {shown}");
  }
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
  assert!(
    screen.exists(&format!("orchestrator-composer-choice-{webhooks}")),
    "the module is a chip"
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
fn orchestrator_watch_module_filters_by_name_and_ctrl_enter_watches_with_the_order() {
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
  screen.until("the chip", |screen| {
    screen.exists(&format!("orchestrator-composer-choice-{webhooks}"))
  });
  screen.set(INPUT, "Ask before merging.");
  screen.key(("Enter", "Enter"), true);
  screen.until("the subscription", |screen| {
    screen
      .db()
      .active_subscriptions()
      .expect("read")
      .of(WatchTarget::Module(webhooks))
      .is_some()
  });
  let subscriptions = screen.db().active_subscriptions().expect("read");
  let led = subscriptions.of(WatchTarget::Module(webhooks)).expect("led");
  assert_eq!(led.chat(), here, "this conversation leads it");
  assert_eq!(led.order(), Some("Ask before merging."));
  screen.until("the chips to go", |screen| !screen.exists(CHIP));
}
