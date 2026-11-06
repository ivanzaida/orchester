//! ORC-781: *Led in* among the task's properties (`IbtAj` in 09 `SLuUW`, `i687ei` b): no row while nobody leads the
//! task; the leading conversation's title with *· via Webhooks v2* once the task's module is watched; a press opens
//! that conversation.

use orchester_app::routes::{OrchestratorChat, ProjectPage, Route, TaskTab};
use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  module::{ModuleDetails, ModuleState},
  name::Name,
  orchestrator::{Chat, Subscription, WatchTarget},
  task::organization::ChangeActor,
  time::Timestamp,
};
use orchester_store::{ProjectDb, WatchStart};

use super::Page;

/// Puts task `key` in a new module Webhooks v2 and has a new conversation watch that module; returns the
/// conversation.
fn watch_its_module(page: &Page, key: &str) -> ChatId {
  let task = page.task(key).id();
  let now = Timestamp::now();
  let mut db = ProjectDb::open(&page.repo).expect("the project");
  let details = ModuleDetails::new(
    Name::new("Webhooks v2").expect("a name"),
    "",
    (None, None),
    ModuleState::Open,
  )
  .expect("details");
  let module = db.create_module(details, now).expect("the module").id();
  db.set_task_module(task, Some(module), ChangeActor::Person, now)
    .expect("in the module");
  let chat = ChatId::generate();
  let body = MessageBody::new("Webhooks v2 and Search index").expect("a body");
  db.keep_chat_message(&Chat::start(chat, &body, now), (ChatMessageId::generate(), &body), now)
    .expect("the conversation");
  let started = db
    .start_subscription(&Subscription::new(chat, WatchTarget::Module(module), "", now))
    .expect("the subscription");
  assert_eq!(started, WatchStart::Started);
  chat
}

#[test]
fn task_led_in_property_names_the_leading_conversation_via_its_module_and_opens_it() {
  let mut page = Page::seeded("task-states");
  let key = page.key("failed");
  page.open(&key, TaskTab::Summary, "task-properties");
  assert!(
    !page.has("task-property-led-in-row"),
    "no row while nobody leads the task"
  );

  let chat = watch_its_module(&page, &key);
  page.changed_outside();
  page.window.until("the Led in row", |window| {
    window.texts("task-property-led-in-row").is_some()
  });
  assert_eq!(page.text("task-property-led-in-key"), "Led in");
  assert_eq!(
    page.text("task-property-led-in-label"),
    "«Webhooks v2 and Search index» · via Webhooks v2"
  );

  page.window.press("task-property-led-in");
  let project = page.project;
  let opened = Route::Project(project, ProjectPage::Orchestrator(OrchestratorChat::Chat(chat)));
  page.window.until("the conversation opened", |window| {
    window.navigation().route_now() == opened
  });
}
