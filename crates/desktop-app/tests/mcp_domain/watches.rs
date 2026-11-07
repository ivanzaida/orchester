//! ORC-770: who leads what, read through the domain tools. `get_watches` gives each Orchestrator conversation's
//! subscriptions (target, kind, since, order) and what waits there now, as `/watching` shows it; `get_module` and
//! `get_task` name the conversation that leads them, with *via*; and no tool writes a subscription. Run with
//! `cargo test -p orchester-app --test mcp_domain domain_watches`.

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, ModuleId},
  orchestrator::{Chat, Subscription, WatchTarget},
  time::Timestamp,
};
use orchester_store::{ProjectDb, WatchStart};
use serde_json::{Value, json};

use super::Served;

/// Kontur with a module Checkout holding the draft Card form, and the draft Receipts in no module; nobody leads them
/// until [`Watched::watch`].
struct Watched {
  served: Served,
  project: String,
  repo: std::path::PathBuf,
  module: ModuleId,
  card: String,
  receipts: String,
}

/// The conversations [`Watched::watch`] made: one leads Checkout, the other the whole project.
struct Leaders {
  lead: ChatId,
  rest: ChatId,
}

impl Watched {
  fn new() -> Self {
    let (mut served, seeded) = Served::seeded("empty");
    let made = served.ok("create_module", &json!({ "project": "Kontur", "name": "Checkout" }));
    let module = made["module"]["id"]
      .as_str()
      .expect("an id")
      .parse()
      .expect("a module id");
    let key = |made: Value| made["task"]["key"].as_str().expect("a key").to_owned();
    let card = key(served.ok(
      "create_task",
      &json!({ "project": "Kontur", "title": "Card form", "module": "Checkout" }),
    ));
    let receipts = key(served.ok("create_task", &json!({ "project": "Kontur", "title": "Receipts" })));
    Self {
      served,
      project: seeded.project.to_string(),
      repo: seeded.repo,
      module,
      card,
      receipts,
    }
  }

  /// Has *Checkout lead* watch Checkout with its own order and *Everything else* the whole project.
  fn watch(&self) -> Leaders {
    let mut db = ProjectDb::open(&self.repo).expect("the project");
    let lead = conversation(&mut db, "Checkout lead");
    subscribe(
      &mut db,
      lead,
      WatchTarget::Module(self.module),
      "Answer agents from the brief.",
    );
    let rest = conversation(&mut db, "Everything else");
    subscribe(&mut db, rest, WatchTarget::Project, "");
    Leaders { lead, rest }
  }
}

/// A new conversation titled `title`.
fn conversation(db: &mut ProjectDb, title: &str) -> ChatId {
  let (chat, now) = (ChatId::generate(), Timestamp::now());
  let body = MessageBody::new(title).expect("a body");
  db.keep_chat_message(&Chat::start(chat, &body, now), (ChatMessageId::generate(), &body), now)
    .expect("the conversation");
  chat
}

fn subscribe(db: &mut ProjectDb, chat: ChatId, target: WatchTarget, order: &str) {
  let started = db
    .start_subscription(&Subscription::new(chat, target, order, Timestamp::now()))
    .expect("the subscription");
  assert_eq!(started, WatchStart::Started);
}

/// The conversation of `watches` titled `title`.
fn titled<'a>(watches: &'a Value, title: &str) -> &'a Value {
  watches["conversations"]
    .as_array()
    .and_then(|conversations| conversations.iter().find(|conversation| conversation["title"] == title))
    .unwrap_or_else(|| panic!("{title} in {watches}"))
}

#[test]
fn domain_watches_lists_each_conversations_subscriptions_and_what_waits_there() {
  let mut watched = Watched::new();
  let none = watched.served.ok("get_watches", &json!({ "project": "Kontur" }));
  assert_eq!(none["conversations"], json!([]), "{none}");
  let Leaders { lead, .. } = watched.watch();

  let watches = watched.served.ok("get_watches", &json!({ "project": "Kontur" }));
  assert_eq!(watches["conversations"].as_array().map(Vec::len), Some(2), "{watches}");
  let checkout = titled(&watches, "Checkout lead");
  assert_eq!(checkout["id"], lead.to_string());
  assert_eq!(
    checkout["link"],
    format!("/project/{}/orchestrator?chat={lead}", watched.project)
  );
  assert_eq!(checkout["leads"]["modules"], 1, "{checkout}");
  assert_eq!(checkout["leads"]["shown"], json!(["1 module"]));
  assert_eq!(checkout["total"], json!(["Nothing waits for you"]));
  let subscription = &checkout["subscriptions"][0];
  assert_eq!(
    subscription["target"],
    json!({
      "kind": "module",
      "id": watched.module.to_string(),
      "name": "Checkout",
      "link": format!("/project/{}/modules/{}", watched.project, watched.module),
    })
  );
  assert!(subscription["since"].is_i64(), "{subscription}");
  assert_eq!(
    subscription["order"],
    json!({ "source": "own", "text": "Answer agents from the brief." })
  );
  assert_eq!(subscription["tasks"], json!([]), "a draft has nothing under way");
  assert_eq!(subscription["rest"], "1 more task with nothing under way");
  let project = &titled(&watches, "Everything else")["subscriptions"][0];
  assert_eq!(project["target"]["kind"], "project", "{project}");
  assert_eq!(project["order"]["source"], "none");
  assert_eq!(
    project["rest"], "1 more task with nothing under way",
    "Card form is led by its module's subscription alone"
  );
}

#[test]
fn domain_watches_reads_one_conversation_by_id_or_title() {
  let mut watched = Watched::new();
  let Leaders { rest, .. } = watched.watch();
  let mut db = ProjectDb::open(&watched.repo).expect("the project");
  conversation(&mut db, "Twice");
  conversation(&mut db, "Twice");
  drop(db);
  let served = &mut watched.served;
  let one = served.ok(
    "get_watches",
    &json!({ "project": "Kontur", "conversation": "checkout LEAD" }),
  );
  assert_eq!(one["conversations"].as_array().map(Vec::len), Some(1), "{one}");
  let by_id = served.ok(
    "get_watches",
    &json!({ "project": "Kontur", "conversation": rest.to_string() }),
  );
  assert_eq!(by_id["conversations"][0]["title"], "Everything else", "{by_id}");
  let twice = served.refused("get_watches", &json!({ "project": "Kontur", "conversation": "Twice" }));
  assert_eq!(twice["code"], "conversation_ambiguous", "{twice}");
  let unknown = served.refused(
    "get_watches",
    &json!({ "project": "Kontur", "conversation": "Nobody here" }),
  );
  assert_eq!(unknown["code"], "conversation_not_found", "{unknown}");
}

#[test]
fn domain_watches_get_module_and_get_task_name_the_leading_conversation() {
  let mut watched = Watched::new();
  let (card, receipts) = (watched.card.clone(), watched.receipts.clone());
  let served = &mut watched.served;
  let page = served.ok("get_module", &json!({ "project": "Kontur", "module": "Checkout" }));
  assert!(page["led_in"].is_null(), "nobody leads it yet: {page}");
  let task = served.ok("get_task", &json!({ "project": "Kontur", "key": receipts }));
  assert!(task["led_in"].is_null(), "nobody leads it yet: {task}");
  let Leaders { lead, rest } = watched.watch();
  let served = &mut watched.served;

  let page = served.ok("get_module", &json!({ "project": "Kontur", "module": "Checkout" }));
  let led = &page["led_in"];
  assert_eq!(led["conversation"]["id"], lead.to_string(), "{page}");
  assert_eq!(led["conversation"]["title"], "Checkout lead");
  assert!(led["via"].is_null(), "the module's own subscription: {led}");
  assert_eq!(led["shown"], "«Checkout lead»");
  let task = served.ok("get_task", &json!({ "project": "Kontur", "key": card }));
  let led = &task["led_in"];
  assert_eq!(led["conversation"]["title"], "Checkout lead", "{task}");
  assert_eq!(led["via"]["kind"], "module");
  assert_eq!(led["via"]["id"], watched.module.to_string());
  assert_eq!(led["shown"], "«Checkout lead» · via Checkout");
  let task = served.ok("get_task", &json!({ "project": "Kontur", "key": receipts }));
  let led = &task["led_in"];
  assert_eq!(led["conversation"]["id"], rest.to_string(), "{task}");
  assert_eq!(led["via"]["kind"], "project");
  assert_eq!(led["shown"], "«Everything else» · via the whole project");

  let tools = served.tools();
  let watch_tools: Vec<&Value> = tools
    .iter()
    .filter(|tool| {
      let name = tool["name"].as_str().unwrap_or_default();
      name.contains("watch") || name.contains("subscri") || name.contains("lead")
    })
    .collect();
  assert_eq!(
    watch_tools.len(),
    1,
    "get_watches alone, no tool writes: {watch_tools:?}"
  );
  assert_eq!(watch_tools[0]["name"], "get_watches");
  assert_eq!(watch_tools[0]["annotations"]["readOnlyHint"], true);
}
