//! The `orchestrator` scenario: its waits stand as the conversation reads them, under the keys the app's follower
//! keeps them by, so following the project keeps nothing new; and the runs-on picker's sign-ins, with their checks and
//! plan reads, are in the application database. `orchestrator-leads` has its subscriptions standing.

use orchester_core::{
  account::AccountState,
  id::ChatId,
  orchestrator::{Chat, Subscription, WatchTarget},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::AppDb;
use orchester_workspace::orchestrator::{
  chat::{ChatView, EndMoved},
  waits::{MergeChecks, Switches, follow_in},
};

use super::Seed;
use crate::kontur::scenarios::orchestrator::leads;

/// The conversation of `seed` titled `title`, as the screen reads it.
fn conversation(seed: &Seed, title: &str) -> ChatView {
  let chats = seed.db().chats().expect("the conversations");
  let chat: ChatId = chats
    .iter()
    .find(|listing| listing.chat.title() == title)
    .map(|listing| listing.chat.id())
    .unwrap_or_else(|| panic!("a conversation {title}"));
  ChatView::load_in(seed.repo(), chat).expect("read").expect("kept")
}

#[test]
fn seed_orchestrator_keeps_waits_that_stand_under_the_followers_keys() {
  let seed = Seed::of("orchestrator");
  let away = conversation(&seed, "Plan the webhook retry work");
  assert_eq!(
    away.standing.len(),
    4,
    "the digest's question and three interrupted runs stand"
  );
  let cards = conversation(&seed, "Every card");
  assert_eq!(
    cards.standing.len(),
    3,
    "the run ready to merge, the plan and the question being read stand"
  );
  assert_eq!(
    cards.moved.values().copied().collect::<Vec<_>>(),
    vec![EndMoved::Merged],
    "the end of the run merged since no longer waits: its line is neutral"
  );
  // Only subscriptions route the waits (ORC-895): with everything watched, a wait kept under another key would be
  // kept again.
  let mut db = seed.db();
  let now = Timestamp::now();
  let everything = Chat::named(ChatId::generate(), "Everything", now);
  db.keep_watch_entries(&everything, &[], now).expect("the conversation");
  let watching = Subscription::new(everything.id(), WatchTarget::Project, "", now);
  db.start_subscription(&watching).expect("watching");
  let followed = follow_in(
    (seed.repo(), seed.seeded.project, &Programs::processes()),
    &MergeChecks::default(),
    (Switches::default(), true),
  )
  .expect("followed");
  assert!(
    !followed.kept,
    "every wait and end is kept under the follower's own key"
  );
}

#[test]
fn seed_orchestrator_adds_the_pickers_sign_ins_with_their_plan_use() {
  let seed = Seed::of("orchestrator");
  let app = AppDb::open(&seed.seeded.app_database).expect("app database");
  let accounts = app.agent_accounts().expect("accounts");
  let state = |named: &str| {
    let account = accounts
      .iter()
      .find(|account| account.name().is_some_and(|name| name.as_str() == named))
      .unwrap_or_else(|| panic!("an account {named}"));
    let read = app.last_plan_read(account.id()).expect("plan read");
    (account.state(), read.is_some())
  };
  assert_eq!(state("Work"), (AccountState::SignedIn, true));
  assert_eq!(state("Personal"), (AccountState::SignedIn, true));
  assert_eq!(state("Client"), (AccountState::NotSignedIn, false));
}

#[test]
fn seed_orchestrator_leads_has_led_modules_a_task_the_project_and_an_already_led_target() {
  let seed = Seed::of("orchestrator-leads");
  let db = seed.db();
  let leading = conversation(&seed, leads::LEADING);
  let billing = conversation(&seed, leads::BILLING);
  let subscriptions = db.active_subscriptions().expect("the subscriptions");
  let led: Vec<(WatchTarget, &str)> = subscriptions
    .of_chat(leading.chat.id())
    .map(|subscription| (subscription.target(), subscription.order().unwrap_or("")))
    .collect();
  assert_eq!(led.len(), 3, "a module, a task and the project: {led:?}");
  assert!(
    led
      .iter()
      .any(|(target, order)| matches!(target, WatchTarget::Module(_)) && *order == leads::WEBHOOKS_ORDER),
    "Webhooks v2 with its order: {led:?}"
  );
  assert!(
    led.iter().any(|(target, _)| matches!(target, WatchTarget::Task(_))),
    "the task: {led:?}"
  );
  let project = subscriptions.of(WatchTarget::Project).expect("the project is led");
  assert_eq!(project.chat(), leading.chat.id());
  assert_eq!(project.order(), None, "the project's default order applies");
  let other: Vec<_> = subscriptions.of_chat(billing.chat.id()).collect();
  assert_eq!(other.len(), 1, "Billing is led in the other conversation");
  assert!(matches!(other[0].target(), WatchTarget::Module(_)));
  assert_eq!(other[0].order(), Some(leads::BILLING_ORDER));
  assert_eq!(
    db.orchestrator_watch().expect("watch settings").default_order(),
    leads::DEFAULT_ORDER
  );
  let latest = db.chats().expect("the conversations");
  let newest = latest
    .iter()
    .max_by_key(|listing| listing.last)
    .expect("a conversation");
  assert_eq!(
    newest.chat.id(),
    leading.chat.id(),
    "the Orchestrator opens on the leading conversation"
  );
}

/// `orchestrator-requests` (2026-11-11): five tool requests kept under the follower's keys, each where the chat's
/// states a–e need it: two still wait (one being read, one left to the person), the Orchestrator allowed one and
/// denied one, and the person allowed one once.
#[test]
fn seed_orchestrator_requests_keep_a_request_in_each_state() {
  use orchester_core::run::{DecidedBy, ToolApprovalDecision};

  let seed = Seed::of("orchestrator-requests");
  let shown = conversation(&seed, "Tool requests of Webhooks v2");
  assert_eq!(shown.requests.len(), 5, "{:?}", shown.requests);
  assert_eq!(
    shown.standing.len(),
    2,
    "the request being read and the one left to the person wait"
  );
  let decided: Vec<_> = shown
    .requests
    .values()
    .filter_map(|request| request.decided)
    .map(|decided| (decided.decision, decided.by))
    .collect();
  for expected in [
    (Some(ToolApprovalDecision::Approved), Some(DecidedBy::Automation)),
    (Some(ToolApprovalDecision::Denied), Some(DecidedBy::AutomationDeny)),
    (Some(ToolApprovalDecision::Approved), Some(DecidedBy::Person)),
  ] {
    assert!(decided.contains(&expected), "{expected:?} in {decided:?}");
  }
}
