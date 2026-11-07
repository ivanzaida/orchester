//! ORC-782 (`i687ei` c): a History row of a conversation that leads something says what it leads after the eye,
//! *Leads 2 modules, 1 task · 12:20*, instead of how many messages it holds; one that leads nothing keeps its message
//! count. Run with `cargo test -p orchester-app --test orchestrator history_leads`.

use orchester_app::routes::OrchestratorChat;
use orchester_core::{
  id::IdempotencyKey,
  module::{ModuleDetails, ModuleState},
  name::Name,
  orchestrator::{Subscription, WatchTarget},
  store::TaskStore,
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_store::{ProjectDb, WatchStart};

use super::{
  harness::{Orchestrator, Setup},
  history::kept,
};

fn module(db: &mut ProjectDb, name: &str) -> orchester_core::id::ModuleId {
  let details =
    ModuleDetails::new(Name::new(name).expect("name"), "", (None, None), ModuleState::Open).expect("details");
  db.create_module(details, Timestamp::now()).expect("module").id()
}

#[test]
fn history_leads_says_what_a_leading_conversation_leads_after_the_eye() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let now = Timestamp::now();
  let earlier = Timestamp::from_unix_millis(now.unix_millis() - 60_000);
  let quiet = kept(&screen, "Why did KON-11 fail?", earlier, "Its token expired.");
  let leading = kept(&screen, "Webhooks v2 and Search index", now, "Done.");
  let mut db = screen.db();
  let webhooks = module(&mut db, "Webhooks v2");
  let search = module(&mut db, "Search index");
  let details = TaskDetails {
    title: Name::new("Delivery log export").expect("title"),
    brief: "Export the delivery log.".into(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::new(None, None).expect("no limits"),
  };
  let task = db
    .create_tasks(IdempotencyKey::generate(), vec![details], now)
    .expect("task")[0]
    .id();
  for target in [
    WatchTarget::Module(webhooks),
    WatchTarget::Module(search),
    WatchTarget::Task(task),
  ] {
    let started = db
      .start_subscription(&Subscription::new(leading, target, "", now))
      .expect("kept");
    assert_eq!(started, WatchStart::Started);
  }
  screen.go(OrchestratorChat::Latest);
  screen.until("the latest conversation", |screen| {
    screen.text("orchestrator-conversation").contains("Done.")
  });
  screen.click("orchestrator-history");
  screen.until("History", |screen| screen.exists("orchestrator-history-panel"));
  let leads = format!("orchestrator-history-leads-{leading}");
  screen.until("the leading row", |screen| screen.exists(&leads));
  let meta = screen.text(&format!("orchestrator-history-meta-{leading}"));
  assert!(
    meta.starts_with("Leads 2 modules, 1 task · "),
    "what it leads, then when: {meta}"
  );
  assert!(!meta.contains("message"), "no message count: {meta}");
  assert!(!screen.exists(&format!("orchestrator-history-leads-{quiet}")));
  let other = screen.text(&format!("orchestrator-history-meta-{quiet}"));
  assert!(
    other.ends_with("2 messages"),
    "a row that leads nothing keeps its count: {other}"
  );
}
