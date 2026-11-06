//! The header's *Leads* (`X93D8T` a–d): nothing while the conversation leads nothing, a chip per subscription that
//! opens its module or task, *+N* for those the header has no room for, and × under the pointer, which stops that
//! subscription at once.

use orchester_app::routes::{OrchestratorChat, ProjectPage, Route};
use orchester_core::{
  id::{ChatId, IdempotencyKey, ModuleId, TaskId},
  module::{ModuleDetails, ModuleState},
  name::Name,
  orchestrator::{Subscription, WatchTarget},
  store::TaskStore,
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
};

use super::{
  harness::{Orchestrator, Setup},
  history::kept,
};

fn module(screen: &Orchestrator, name: &str) -> ModuleId {
  let details =
    ModuleDetails::new(Name::new(name).expect("name"), "", (None, None), ModuleState::Open).expect("details");
  screen
    .db()
    .create_module(details, Timestamp::now())
    .expect("module")
    .id()
}

/// A task titled `title`; its id and key.
fn task(screen: &Orchestrator, title: &str) -> (TaskId, String) {
  let details = TaskDetails {
    title: Name::new(title).expect("title"),
    brief: "Retry them.".into(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::new(None, None).expect("no limits"),
  };
  let task = screen
    .db()
    .create_tasks(IdempotencyKey::generate(), vec![details], Timestamp::now())
    .expect("task")
    .remove(0);
  (task.id(), task.key().to_string())
}

fn watch(screen: &Orchestrator, chat: ChatId, target: WatchTarget, order: &str) {
  screen
    .db()
    .start_subscription(&Subscription::new(chat, target, order, Timestamp::now()))
    .expect("watched");
}

/// Rests the pointer on the middle of `id`.
fn hover(screen: &mut Orchestrator, id: &str) {
  let frame = screen.bounds(id);
  screen
    .window
    .tree
    .mouse_move(frame.x + frame.width / 2.0, frame.y + frame.height / 2.0);
  screen.window.pass();
}

#[test]
fn orchestrator_leads_chips_open_their_target_and_x_stops_one() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let idle = kept(&screen, "Nothing watched here", Timestamp::now(), "Fine.");
  let chat = kept(&screen, "Webhooks v2 and Search index", Timestamp::now(), "On it.");
  let webhooks = module(&screen, "Webhooks v2");
  let (retry, key) = task(&screen, "Retry failed deliveries");
  watch(&screen, chat, WatchTarget::Module(webhooks), "");
  watch(&screen, chat, WatchTarget::Task(retry), "Ask me before merging.");

  screen.go(OrchestratorChat::Chat(idle));
  screen.until("the idle conversation", |screen| {
    screen.text("orchestrator-title") == "Nothing watched here" && screen.text("orchestrator-meta").contains("today")
  });
  assert!(
    !screen.exists("orchestrator-leads"),
    "a conversation that leads nothing shows no Leads"
  );

  screen.go(OrchestratorChat::Chat(chat));
  let module_chip = format!("orchestrator-leads-module-{webhooks}");
  let task_chip = format!("orchestrator-leads-task-{retry}");
  screen.until("the Leads chips", |screen| screen.exists(&task_chip));
  assert!(screen.text("orchestrator-leads-label") == "Leads");
  assert_eq!(screen.text(&format!("{module_chip}-name")), "Webhooks v2");
  assert_eq!(screen.text(&format!("{task_chip}-name")), key);
  assert!(!screen.exists("orchestrator-leads-more"), "two chips fit");
  assert!(
    !screen.exists(&format!("{task_chip}-stop")),
    "× shows only under the pointer"
  );

  hover(&mut screen, &task_chip);
  screen.until("the chip's ×", |screen| screen.exists(&format!("{task_chip}-stop")));
  screen.click(&format!("{task_chip}-stop"));
  screen.until("the task's chip to go", |screen| !screen.exists(&task_chip));
  let led: Vec<WatchTarget> = screen
    .db()
    .active_subscriptions()
    .expect("subscriptions")
    .of_chat(chat)
    .map(Subscription::target)
    .collect();
  assert_eq!(led, [WatchTarget::Module(webhooks)], "× stopped only the task");
  assert_eq!(
    screen.route(),
    Route::Project(screen.project, ProjectPage::Orchestrator(OrchestratorChat::Chat(chat))),
    "× opens nothing"
  );

  screen.click(&module_chip);
  screen.until(
    "the module's page",
    |screen| matches!(screen.route(), Route::Project(_, ProjectPage::Module(id, _)) if id == webhooks),
  );
}

#[test]
fn orchestrator_leads_fold_what_does_not_fit_into_more() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let chat = kept(&screen, "Everything at once", Timestamp::now(), "On it.");
  let total = 12;
  for number in 0..total {
    let id = module(&screen, &format!("A module with a rather long name, number {number}"));
    watch(&screen, chat, WatchTarget::Module(id), "");
  }
  screen.go(OrchestratorChat::Chat(chat));
  screen.until("+N", |screen| screen.exists("orchestrator-leads-more"));
  // The fold settles once the header has room for what shows: its last control stays inside it.
  screen.until("the fold to settle", |screen| {
    let header = screen.bounds("orchestrator-header");
    let history = screen.bounds("orchestrator-history");
    history.x + history.width <= header.x + header.width
  });
  let more = screen.text("orchestrator-leads-more");
  let folded: usize = more
    .trim_start_matches('+')
    .parse()
    .unwrap_or_else(|_| panic!("+N says how many: {more}"));
  assert!(folded > 0 && folded < total, "{folded} of {total} fold");
  let title = screen.bounds("orchestrator-title");
  assert!(title.width > 0.0, "the title keeps its room");
}
