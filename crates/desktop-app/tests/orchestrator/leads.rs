//! The header's *Leads* (`X93D8T` a–d): nothing while the conversation leads nothing, a chip per subscription that
//! opens its module or task, *+N* for those the header has no room for, and × under the pointer, which stops that
//! subscription at once.

use std::time::{Duration, Instant};

use orchester_app::{
  routes::{OrchestratorChat, ProjectPage, Route},
  ui::tooltip::TOOLTIP_DELAY,
};
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

pub(super) fn module(screen: &Orchestrator, name: &str) -> ModuleId {
  let details =
    ModuleDetails::new(Name::new(name).expect("name"), "", (None, None), ModuleState::Open).expect("details");
  screen
    .db()
    .create_module(details, Timestamp::now())
    .expect("module")
    .id()
}

/// A task titled `title`; its id and key.
pub(super) fn task(screen: &Orchestrator, title: &str) -> (TaskId, String) {
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

pub(super) fn watch(screen: &Orchestrator, chat: ChatId, target: WatchTarget, order: &str) {
  screen
    .db()
    .start_subscription(&Subscription::new(chat, target, order, Timestamp::now()))
    .expect("watched");
}

/// Rests the pointer on the middle of `id`.
pub(super) fn hover(screen: &mut Orchestrator, id: &str) {
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
  right_after_the_meta(&mut screen, &module_chip);
  assert_eq!(screen.text(&format!("{module_chip}-name")), "Webhooks v2");
  assert_eq!(screen.text(&format!("{task_chip}-name")), key);
  assert!(!screen.exists("orchestrator-leads-more"), "two chips fit");
  assert!(
    !screen.exists(&format!("{task_chip}-stop")),
    "× shows only under the pointer"
  );

  hover(&mut screen, &task_chip);
  screen.until("the chip's ×", |screen| screen.exists(&format!("{task_chip}-stop")));
  screen
    .window
    .tree
    .tick_timers_at(Instant::now() + TOOLTIP_DELAY + Duration::from_millis(50));
  let tooltip = format!("{task_chip}-tooltip");
  screen.until("the chip's tooltip", |screen| screen.exists(&tooltip));
  let (chip, tip, header) = (
    screen.bounds(&task_chip),
    screen.bounds(&tooltip),
    screen.bounds("orchestrator-header"),
  );
  assert!(
    (tip.x - chip.x).abs() < 1.0,
    "the tooltip starts at its chip's start: {} and {}",
    tip.x,
    chip.x
  );
  assert!(
    (tip.y - (header.y + header.height + 8.0)).abs() < 1.0,
    "the tooltip hangs 8 under the header: {} under {}",
    tip.y,
    header.y + header.height
  );
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

/// What the conversation leads sits right after when it started, 8 from it: the eye, then the chips 6 from it, with no
/// word before them, and the header's actions after the free room.
fn right_after_the_meta(screen: &mut Orchestrator, first_chip: &str) {
  assert!(!screen.exists("orchestrator-leads-label"), "no *Leads* word");
  let (meta, group, eye, chip, new) = (
    screen.bounds("orchestrator-meta"),
    screen.bounds("orchestrator-leads"),
    screen.bounds("orchestrator-leads-eye"),
    screen.bounds(first_chip),
    screen.bounds("orchestrator-new"),
  );
  let after_meta = meta.x + meta.width + 8.0;
  assert!(
    (group.x - after_meta).abs() < 1.0,
    "the group starts 8 after the meta: {} and {after_meta}",
    group.x
  );
  assert!((eye.x - group.x).abs() < 1.0, "the eye comes first");
  assert!(
    (chip.x - (eye.x + eye.width + 6.0)).abs() < 1.0,
    "the first chip is 6 after the eye: {} after {}",
    chip.x,
    eye.x + eye.width
  );
  assert!(group.x + group.width < new.x, "free room before *New*");
}

/// Once the fold settled, the group ends before *New* and what shows stays put.
fn settled(screen: &mut Orchestrator) {
  for _ in 0..2000 {
    screen.window.tree.tick_futures();
    screen.window.pass();
    let (group, new) = (screen.bounds("orchestrator-leads"), screen.bounds("orchestrator-new"));
    if group.x + group.width <= new.x {
      break;
    }
    std::thread::sleep(std::time::Duration::from_millis(5));
  }
  let (group, new) = (screen.bounds("orchestrator-leads"), screen.bounds("orchestrator-new"));
  assert!(
    group.x + group.width <= new.x,
    "the fold settles before *New*: {group:?} {new:?} {:?} {:?}",
    screen.bounds("orchestrator-title"),
    screen.bounds("orchestrator-meta")
  );
  let before = screen.text("orchestrator-leads-more");
  for _ in 0..10 {
    screen.window.tree.tick_futures();
    screen.window.pass();
  }
  assert_eq!(screen.text("orchestrator-leads-more"), before, "the fold holds still");
}

#[test]
fn orchestrator_leads_fold_in_a_narrow_window_and_come_back_in_a_wide_one() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let chat = kept(&screen, "Search", Timestamp::now(), "On it.");
  let modules: Vec<ModuleId> = (0..4)
    .map(|number| module(&screen, &format!("Module number {number}")))
    .collect();
  for id in &modules {
    watch(&screen, chat, WatchTarget::Module(*id), "");
  }
  screen.go(OrchestratorChat::Chat(chat));
  let first = format!("orchestrator-leads-module-{}", modules[0]);
  screen.until("the chips", |screen| screen.exists(&first));
  settled(&mut screen);
  assert!(!screen.exists("orchestrator-leads-more"), "four short chips fit 1440");

  screen.window.tree.resize(760, 900);
  screen.until("+N in a narrow window", |screen| {
    screen.exists("orchestrator-leads-more")
  });
  settled(&mut screen);
  assert!(screen.exists(&first), "at least the first chip shows");
  right_after_the_meta(&mut screen, &first);

  screen.window.tree.resize(1800, 900);
  screen.until("every chip back in a wide one", |screen| {
    !screen.exists("orchestrator-leads-more")
      && modules
        .iter()
        .all(|id| screen.exists(&format!("orchestrator-leads-module-{id}")))
  });
}

#[test]
fn orchestrator_leads_keep_the_actions_in_place_beside_a_long_title() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.window.tree.resize(1000, 900);
  // A title is cut to 80 characters when kept; a window 1000 wide has no room for all of it beside the chips.
  let title = "A conversation whose title goes on and on about webhooks, retries, search indexes and billing";
  let chat = kept(&screen, title, Timestamp::now(), "On it.");
  let modules: Vec<ModuleId> = (0..3)
    .map(|number| module(&screen, &format!("Module number {number}")))
    .collect();
  for id in &modules {
    watch(&screen, chat, WatchTarget::Module(*id), "");
  }
  screen.go(OrchestratorChat::Chat(chat));
  let first = format!("orchestrator-leads-module-{}", modules[0]);
  screen.until("the chips", |screen| screen.exists(&first));
  settled(&mut screen);
  let (header, title, meta, group) = (
    screen.bounds("orchestrator-header"),
    screen.bounds("orchestrator-title"),
    screen.bounds("orchestrator-meta"),
    screen.bounds("orchestrator-leads"),
  );
  assert!(
    title.width > 0.0 && title.x + title.width <= meta.x,
    "the title is cut before the meta"
  );
  assert!(meta.x + meta.width <= group.x, "the meta before the group");
  let mut end = group.x + group.width;
  for action in ["orchestrator-new", "orchestrator-history", "orchestrator-more"] {
    let bounds = screen.bounds(action);
    assert!(bounds.x >= end, "{action} is not overlapped: {} before {end}", bounds.x);
    end = bounds.x + bounds.width;
  }
  assert!(end <= header.x + header.width, "the actions stay inside the header");
}
