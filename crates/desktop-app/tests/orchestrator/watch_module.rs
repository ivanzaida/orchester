//! `/watch-module` (ORC-772, `02 · Choosing modules` `c8pALl` in `Orchestrator watch · redesign`; its picker v2,
//! ORC-889, `P1` `QXPDY`): its picker lists the project's open modules with what they take in, *Led in «…» ↗* for one
//! another conversation leads (which opens it), a module with nothing left to watch behind one counted row, then
//! *Also* with the tasks without a module and everything in the project, and the keys; Enter makes the focused module a
//! chip and closes the picker; the words typed in its own search filter the modules by name; Ctrl Enter has the
//! conversation watch the chips with the words after them as their order. The list's cap, the finished row and the
//! keys of v2 are `watch_picker`'s.

use lurq::app::events::with_synthetic_input;
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
pub(super) const PICKER: &str = "orchestrator-composer-watch-module-picker";
pub(super) const SEARCH: &str = "orchestrator-composer-watch-module-search";
pub(super) const CHIP: &str = "orchestrator-composer-command-chip";

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

/// Kontur's modules: Webhooks v2 (two tasks), Billing (one, led in *Billing and invoices*) and Mobile release (one,
/// Done); one task without a module. The screen shows the conversation *Split Webhooks v2 and Billing*.
pub(super) struct Kontur {
  pub(super) screen: Orchestrator,
  pub(super) here: ChatId,
  pub(super) billing_chat: ChatId,
  pub(super) webhooks: ModuleId,
  pub(super) billing: ModuleId,
  pub(super) mobile: ModuleId,
}

fn kontur() -> Kontur {
  kontur_with(|_| {})
}

/// [`kontur`], with what `more` adds to the project's database before the screen shows it.
pub(super) fn kontur_with(more: impl FnOnce(&mut ProjectDb)) -> Kontur {
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
  let (mobile, mobile_tasks) = module(&mut db, "Mobile release", &["Store screenshots"]);
  done(&mut db, mobile_tasks[0]);
  task(&mut db, "Shared rate limiter");
  let led = Subscription::new(billing_chat, WatchTarget::Module(billing), "", Timestamp::now());
  db.start_subscription(&led).expect("led");
  more(&mut db);
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
    mobile,
  }
}

/// Task `id` is Done.
pub(super) fn done(db: &mut ProjectDb, id: TaskId) {
  db.close_task_organization(id, WorkflowStatus::Done, (ChangeActor::Person, Timestamp::now()))
    .expect("done");
}

/// *Watch modules* chosen in the slash menu, where it is the first command `/watch` matches that can run: the command
/// chip and its picker.
pub(super) fn open_picker(screen: &mut Orchestrator) {
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

pub(super) fn row(id: impl std::fmt::Display) -> String {
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
  assert!(screen.exists(SEARCH), "the picker's own search field");
  let shown = screen.text(PICKER).replace('\u{a0}', " ");
  assert!(shown.contains("Modules 2 open"), "two open modules: {shown}");
  for words in [
    "Webhooks v2",
    "2 tasks · none running",
    "Billing",
    "Led in «Billing and invoices» ↗",
    "1 finished module · type to find it",
    "Also",
    "Tasks without a module",
    "Everything in",
    "What no other subscription takes",
    "Move",
    "Pick and close",
    "Shift Enter",
    "Add another",
    "Close",
  ] {
    assert!(shown.contains(words), "{words}: {shown}");
  }
  let webhooks_row = screen.text(&row(webhooks));
  assert!(
    webhooks_row.contains("Enter"),
    "the focus starts on the first module: {webhooks_row}"
  );
  let order: Vec<f32> = ["Webhooks v2", "Billing", "finished module", "Tasks without a module"]
    .iter()
    .map(|words| {
      let found = screen.text(PICKER).replace('\u{a0}', " ").find(words).expect("shown");
      found as f32
    })
    .collect();
  assert!(
    order.windows(2).all(|pair| pair[0] < pair[1]),
    "in P1's order: {order:?}"
  );
  assert!(
    (screen.bounds(PICKER).width - 560.0).abs() < 0.5,
    "560 wide: {:?}",
    screen.bounds(PICKER)
  );

  screen.key(("Enter", "Enter"), false);
  screen.until("Webhooks v2 a chip", |screen| {
    screen.exists(&format!("orchestrator-composer-choice-{webhooks}"))
  });
  screen.until("the picker closed", |screen| !screen.exists(PICKER));

  screen.click(CHIP);
  screen.until("Webhooks v2 added", |screen| {
    screen.exists(&row(webhooks)) && screen.text(&row(webhooks)).contains("Added")
  });
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
  screen.set(SEARCH, "hoo");
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

#[test]
fn orchestrator_watch_module_refuses_synthetic_ctrl_enter_and_keeps_the_chips_and_the_order() {
  let Kontur {
    mut screen,
    here,
    webhooks,
    ..
  } = kontur();
  open_picker(&mut screen);
  screen.until("the modules", |screen| screen.exists(&row(webhooks)));
  screen.key(("Enter", "Enter"), false);
  let chip = format!("orchestrator-composer-choice-{webhooks}");
  screen.until("the chip", |screen| screen.exists(&chip));
  screen.set(INPUT, "Ask before merging.");

  with_synthetic_input(|| screen.key(("Enter", "Enter"), true));
  let problem = "orchestrator-watch-problem";
  screen.until("the person-only notice", |screen| {
    screen.exists(problem) && screen.text(problem).contains("Only you can start watching")
  });
  assert!(
    screen
      .db()
      .active_subscriptions()
      .expect("read")
      .of(WatchTarget::Module(webhooks))
      .is_none(),
    "synthetic input watches nothing"
  );
  assert!(screen.exists(CHIP) && screen.exists(&chip), "the chips stay");

  // The person's own Ctrl Enter, back in the message field, applies what stayed, the order included.
  screen
    .window
    .tree
    .get_element_by_id_mut(INPUT)
    .expect("the message field")
    .focus();
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
  assert_eq!(led.order(), Some("Ask before merging."), "the order stayed");
}
