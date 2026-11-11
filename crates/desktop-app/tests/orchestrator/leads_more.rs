//! *+N* of the header's Leads (`R8tXP` in `X93D8T` c) and the list it opens (`Leads popover` `Jl9jP` in `nF6Ro`): a
//! control the pointer and the keyboard press, a list of every target the conversation leads with since when, a row
//! that opens its target, an `x` that stops only its subscription, a refused stop said in the list, and the list
//! closing by itself once *+N* goes.

use orchester_app::routes::{OrchestratorChat, ProjectPage, Route};
use orchester_core::{
  id::{ChatId, ModuleId},
  orchestrator::{Subscription, WatchTarget},
  time::Timestamp,
};

use super::{
  harness::{Orchestrator, Setup},
  history::kept,
  leads::{module, watch},
};

const MORE: &str = "orchestrator-leads-more";
const LIST: &str = "orchestrator-leads-more-popover";

/// A conversation that leads `count` modules with long names, shown in a window 1000 wide: most fold into *+N*.
fn folded(count: usize) -> (Orchestrator, ChatId, Vec<ModuleId>) {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.window.tree.resize(1000, 900);
  let chat = kept(&screen, "Everything at once", Timestamp::now(), "On it.");
  let modules: Vec<ModuleId> = (0..count)
    .map(|number| module(&screen, &format!("A module with a rather long name, number {number}")))
    .collect();
  for id in &modules {
    watch(&screen, chat, WatchTarget::Module(*id), "");
  }
  screen.go(OrchestratorChat::Chat(chat));
  screen.until("+N", |screen| screen.exists(MORE));
  screen.until("the fold to settle", |screen| {
    let group = screen.bounds("orchestrator-leads");
    group.x + group.width <= screen.bounds("orchestrator-new").x
  });
  (screen, chat, modules)
}

fn row(id: ModuleId) -> String {
  format!("{MORE}-module-{id}")
}

fn led(screen: &Orchestrator, chat: ChatId) -> Vec<WatchTarget> {
  screen
    .db()
    .active_subscriptions()
    .expect("subscriptions")
    .of_chat(chat)
    .map(Subscription::target)
    .collect()
}

#[test]
fn orchestrator_leads_more_opens_and_closes_its_list() {
  let (mut screen, ..) = folded(8);
  assert!(!screen.exists(LIST), "closed at first");

  screen.click(MORE);
  screen.until("the list", |screen| screen.exists(LIST));
  let (more, list, header) = (
    screen.bounds(MORE),
    screen.bounds(LIST),
    screen.bounds("orchestrator-header"),
  );
  // From +N's start, moved back only as far as the window's edge needs.
  let start = more.x.min(1000.0 - 320.0);
  assert!(
    (list.x - start).abs() < 1.0,
    "the list starts at +N's start: {list:?} {more:?}"
  );
  assert!(
    (list.y - (header.y + header.height + 2.0)).abs() < 1.0,
    "the list hangs 2 under the header: {} under {}",
    list.y,
    header.y + header.height
  );
  assert!((list.width - 320.0).abs() < 1.0, "320 wide: {}", list.width);
  screen.click(MORE);
  screen.until("a second press to close it", |screen| !screen.exists(LIST));

  screen.click(MORE);
  screen.until("the list again", |screen| screen.exists(LIST));
  screen.key(("Escape", "Escape"), false);
  screen.until("Esc to close it", |screen| !screen.exists(LIST));

  screen.click(MORE);
  screen.until("the list again", |screen| screen.exists(LIST));
  screen.press("orchestrator-title");
  screen.until("a press outside to close it", |screen| !screen.exists(LIST));

  screen.window.tree.get_element_by_id_mut(MORE).expect("+N").focus();
  screen.key(("Enter", "Enter"), false);
  screen.until("Enter to open it", |screen| screen.exists(LIST));
  screen.key(("Escape", "Escape"), false);
  screen.until("Esc to close it", |screen| !screen.exists(LIST));
  screen.key((" ", "Space"), false);
  screen.until("Space to open it", |screen| screen.exists(LIST));
}

#[test]
fn orchestrator_leads_more_lists_every_target_with_since_when() {
  let (mut screen, _, modules) = folded(8);
  screen.click(MORE);
  screen.until("the list", |screen| screen.exists(LIST));
  assert_eq!(screen.text(&format!("{LIST}-head")), "This conversation leads");
  let mut above = 0.0;
  for (number, id) in modules.iter().enumerate() {
    let row = row(*id);
    assert!(screen.exists(&row), "module {number} is listed, shown or folded");
    assert_eq!(
      screen.text(&format!("{row}-name")),
      format!("A module with a rather long name, number {number}")
    );
    let since = screen.text(&format!("{row}-since"));
    assert!(
      since.starts_with("since ") && since.contains(':'),
      "since today's time: {since}"
    );
    let top = screen.bounds(&row).y;
    assert!(top > above, "in the order they were kept");
    above = top;
  }
}

#[test]
fn orchestrator_leads_more_row_opens_its_module() {
  let (mut screen, _, modules) = folded(8);
  screen.click(MORE);
  screen.until("the list", |screen| screen.exists(LIST));
  let last = *modules.last().expect("a module");
  screen.click(&row(last));
  screen.until(
    "the module's page",
    |screen| matches!(screen.route(), Route::Project(_, ProjectPage::Module(id, _)) if id == last),
  );
}

#[test]
fn orchestrator_leads_more_x_stops_only_its_subscription() {
  let (mut screen, chat, modules) = folded(8);
  screen.click(MORE);
  screen.until("the list", |screen| screen.exists(LIST));
  let gone = modules[3];
  screen.click(&format!("{}-stop", row(gone)));
  screen.until("its row to go", |screen| !screen.exists(&row(gone)));
  let left: Vec<WatchTarget> = modules
    .iter()
    .filter(|id| **id != gone)
    .map(|id| WatchTarget::Module(*id))
    .collect();
  assert_eq!(led(&screen, chat), left, "× stopped only that one");
  assert!(screen.exists(LIST), "the list stays open");
  assert!(
    modules
      .iter()
      .filter(|id| **id != gone)
      .all(|id| screen.exists(&row(*id))),
    "the others stay listed"
  );
  assert_eq!(
    screen.route(),
    Route::Project(screen.project, ProjectPage::Orchestrator(OrchestratorChat::Chat(chat))),
    "× opens nothing"
  );
}

#[test]
fn orchestrator_leads_more_says_a_refused_stop() {
  let (mut screen, chat, modules) = folded(8);
  screen.click(MORE);
  screen.until("the list", |screen| screen.exists(LIST));
  let ended = modules[2];
  // Another change ends it before the press: there is nothing left to stop.
  screen
    .db()
    .stop_subscription(chat, WatchTarget::Module(ended), Timestamp::now())
    .expect("stopped elsewhere");
  screen.click(&format!("{}-stop", row(ended)));
  let refused = format!("{LIST}-refused");
  screen.until("the refusal", |screen| screen.exists(&refused));
  assert!(
    screen.text(&refused).starts_with("Not stopped · "),
    "{}",
    screen.text(&refused)
  );
}

#[test]
fn orchestrator_leads_more_list_closes_when_more_goes() {
  let (mut screen, _, modules) = folded(4);
  screen.click(MORE);
  screen.until("the list", |screen| screen.exists(LIST));
  screen.window.tree.resize(2600, 900);
  screen.until("+N and its list to go", |screen| {
    !screen.exists(MORE) && !screen.exists(LIST)
  });
  screen.window.tree.resize(1000, 900);
  screen.until("+N back", |screen| screen.exists(MORE));
  for _ in 0..5 {
    screen.window.pass();
  }
  assert!(!screen.exists(LIST), "the list stays closed until pressed");
  assert!(
    modules
      .iter()
      .any(|id| screen.exists(&format!("orchestrator-leads-module-{id}")))
  );
}
