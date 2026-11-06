//! The chat's `get_conversation` gives what its conversation leads, read-only (ORC-769): the subscriptions of that
//! conversation that have not ended, each with its target named, the person's order and the order that applies, and
//! whether the asked task's events come through it; another conversation's are left out, an app client's
//! `get_conversation` gives none, and no chat tool changes a subscription.
//! `cargo test -p orchester-workspace --test orchestrator_conversation_leads conversation_leads`.

use std::{fs, sync::Arc};

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, ModuleId, ProjectId},
  name::Name,
  orchestrator::{Chat, Subscription, TurnMode, TurnState, WatchSettings, WatchTarget},
  project::{Project, ProjectKey},
  run::Answerer,
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{AppDb, ProjectDb, WatchStart};
use orchester_workspace::{
  activity::Activity,
  app_mcp::{self, Translate},
  orchestrator::tools::{self, ChatScope},
  runner::Runners,
  secrets::Vault,
  sharing::Servers,
  tasks::{integration_jobs::ProjectJobs, outside::OutsideChanges},
  workspace::Workspace,
};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// No secret is read or kept here.
struct NoVault;

impl SecretVault for NoVault {
  fn store(&self, _: &VaultKey, _: &SecretValue) -> Result<(), VaultError> {
    Err(VaultError::Failed("no secrets in these tests".to_owned()))
  }

  fn read(&self, _: &VaultKey) -> Result<SecretValue, VaultError> {
    Err(VaultError::NotFound)
  }

  fn delete(&self, _: &VaultKey) -> Result<(), VaultError> {
    Ok(())
  }
}

/// The app's session with one local project, Kontur, and a conversation of its chat.
struct Desktop {
  services: app_mcp::Services,
  runners: Runners,
  runtime: Runtime,
  repo: TempDir,
  scope: ChatScope,
  _data: TempDir,
}

impl Desktop {
  fn new() -> Self {
    let repo = TempDir::new().expect("repository");
    fs::create_dir(repo.path().join(".git")).expect("create .git");
    let project = Project::new(
      ProjectId::generate(),
      ProjectKey::new("KON").expect("key"),
      Name::new("Kontur").expect("name"),
    );
    ProjectDb::create(repo.path(), project).expect("project");
    let data = TempDir::new().expect("application data");
    let workspace = Workspace::new(AppDb::open(&data.path().join("orchester.db")).expect("database"));
    workspace.open(repo.path()).expect("register the project");
    let runtime = Runtime::new().expect("runtime");
    let runners = Runners::new(
      repo.path().join("no-runner"),
      runtime.handle().clone(),
      Activity::new(),
      orchester_ipc::SessionEnd::Clean,
    );
    let version = orchester_api::releases::ReleaseVersion::parse("0.5.0").expect("a release version");
    let servers = Servers::new(workspace.clone(), Vault::with(Arc::new(NoVault)), version);
    let services = app_mcp::Services::new(
      workspace.clone(),
      runners.clone(),
      servers,
      Programs::processes(),
      (OutsideChanges::default(), Activity::new(), ProjectJobs::default()),
    );
    let project = workspace.projects().expect("projects")[0].id;
    let scope = ChatScope {
      project,
      name: "Kontur".to_owned(),
      root: repo.path().to_path_buf(),
      chat: ChatId::generate(),
      on_its_own: false,
    };
    let desktop = Self {
      services,
      runners,
      runtime,
      repo,
      scope,
      _data: data,
    };
    desktop.says(desktop.scope.chat, "Lead the delivery work");
    desktop
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(self.repo.path()).expect("project database")
  }

  /// A message `text` of the person's in conversation `chat`, whose turn thinks now.
  fn says(&self, chat: ChatId, text: &str) {
    let mut db = self.db();
    let body = MessageBody::new(text).expect("text");
    let kept = db
      .chat(chat)
      .expect("read")
      .unwrap_or_else(|| Chat::start(chat, &body, Timestamp::now()));
    db.keep_chat_message_in(
      &kept,
      (ChatMessageId::generate(), &body),
      (&[], Answerer::Person),
      (&TurnState::Thinking, Timestamp::now()),
      TurnMode::Acts,
    )
    .expect("kept");
  }

  /// The chat's tool `name` with `arguments`, as its agent calls it.
  fn chat(&self, name: &str, arguments: Value) -> Value {
    let Value::Object(arguments) = arguments else {
      panic!("arguments are an object");
    };
    let translate = Translate::new(|message| message.key.to_owned());
    self
      .runtime
      .block_on(tools::call(
        (&self.services, &translate, &self.runners),
        &self.scope,
        name,
        arguments,
      ))
      .unwrap_or_else(|error| panic!("{name} refused: {}", error.to_json(|message| message.key.to_owned())))
  }

  /// The app's own tool `name` with `arguments`, as any client of the app calls it.
  fn app(&self, name: &str, arguments: Value) -> Value {
    let Value::Object(arguments) = arguments else {
      panic!("arguments are an object");
    };
    let translate = Translate::new(|message| message.key.to_owned());
    self
      .runtime
      .block_on(app_mcp::call(&self.services, &translate, name, arguments))
      .expect("an app tool")
      .unwrap_or_else(|error| panic!("{name} refused: {}", error.to_json(|message| message.key.to_owned())))
  }

  fn module(&self, name: &str) -> ModuleId {
    let made = self.chat("create_module", json!({ "project": "Kontur", "name": name }));
    made["module"]["id"]
      .as_str()
      .expect("an id")
      .parse()
      .expect("a module id")
  }

  /// A task titled `title` in module `module` (by name): its key.
  fn task(&self, title: &str, module: &str) -> String {
    let told = self.chat(
      "create_task",
      json!({ "project": "Kontur", "title": title, "module": module }),
    );
    told["task"]["key"].as_str().expect("a key").to_owned()
  }

  /// `chat` subscribes to `target` with `order`.
  fn watches(&self, chat: ChatId, target: WatchTarget, order: &str) {
    let started = self
      .db()
      .start_subscription(&Subscription::new(chat, target, order, Timestamp::now()))
      .expect("kept");
    assert_eq!(started, WatchStart::Started);
  }
}

#[test]
fn conversation_leads_are_given_by_the_chats_get_conversation_with_the_order_that_applies() {
  let desktop = Desktop::new();
  let delivery = desktop.module("Delivery");
  let card = desktop.task("Card form", "Delivery");
  let mut db = desktop.db();
  db.save_orchestrator_watch(&WatchSettings::new("Report every failure", None).expect("settings"))
    .expect("saved");
  desktop.watches(desktop.scope.chat, WatchTarget::Module(delivery), "Merge when green");
  desktop.watches(desktop.scope.chat, WatchTarget::WithoutModule, "");
  let other = ChatId::generate();
  desktop.says(other, "Billing work");
  desktop.watches(other, WatchTarget::Project, "Only report");

  let told = desktop.chat("get_conversation", json!({ "project": "Kontur", "task": card }));
  let leads = told["subscriptions"]["leads"].as_array().expect("its subscriptions");
  assert_eq!(leads.len(), 2, "only this conversation's: {told}");
  assert_eq!(
    leads[0]["target"],
    json!({ "kind": "module", "id": delivery.to_string() })
  );
  assert_eq!(leads[0]["name"], "Delivery");
  assert_eq!(leads[0]["order"], "Merge when green");
  assert_eq!(leads[0]["order_applies"], "own");
  assert_eq!(leads[0]["applied_order"], "Merge when green");
  assert_eq!(
    leads[0]["task_comes_through"], true,
    "the card's events come through its module"
  );
  assert_eq!(leads[1]["target"], json!({ "kind": "without_module" }));
  assert_eq!(leads[1]["name"], "Tasks without a module");
  assert_eq!(leads[1]["order"], Value::Null);
  assert_eq!(leads[1]["order_applies"], "project_default");
  assert_eq!(leads[1]["applied_order"], "Report every failure");
  assert_eq!(leads[1]["task_comes_through"], false);
  let note = told["subscriptions"]["note"].as_str().expect("a note");
  assert!(note.contains("read-only"), "{note}");
  assert_eq!(
    told["task"]["key"],
    json!(card),
    "the conversation is still there: {told}"
  );
}

#[test]
fn conversation_leads_are_read_only_and_only_the_chat_gets_them() {
  let desktop = Desktop::new();
  desktop.module("Delivery");
  let card = desktop.task("Card form", "Delivery");
  desktop.watches(desktop.scope.chat, WatchTarget::Project, "");
  let told = desktop.app("get_conversation", json!({ "project": "Kontur", "task": card }));
  assert!(told.get("subscriptions").is_none(), "an app client gets none: {told}");
  let named: Vec<String> = tools::listed().into_iter().map(|tool| tool.name.to_string()).collect();
  assert!(
    !named
      .iter()
      .any(|name| name.contains("watch") || name.contains("subscri") || name.contains("lead")),
    "no chat tool changes a subscription: {named:?}"
  );
  let told = desktop.chat("get_conversation", json!({ "project": "Kontur", "task": card }));
  assert_eq!(
    told["subscriptions"]["leads"][0]["order_applies"], "none",
    "no order and no default order: {told}"
  );
  assert_eq!(told["subscriptions"]["leads"][0]["name"], "Kontur");
  assert_eq!(
    desktop.db().chat_subscriptions(desktop.scope.chat).expect("read").len(),
    1,
    "reading changed nothing"
  );
}
