//! A task the Orchestrator chat creates comes back to the conversation that leads (ORC-765): `create_task` in a turn of
//! a conversation that watches something, outside what it watches, makes the task its task subscription with the
//! order of the subscription whose event started the turn (the default order in a person's turn); a task whose module
//! another conversation leads already is not taken, and the answer and the action's row say who leads it.
//! `cargo test -p orchester-workspace --test orchestrator_created created`.

use std::{fs, sync::Arc};

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, ModuleId, ProjectId, RunId},
  name::Name,
  orchestrator::{
    AgentWait, Chat, ChatAction, EntryKind, Subscription, TurnMode, TurnState, WaitKind, WatchTarget,
    watch::CreatedWatch,
  },
  project::{Project, ProjectKey},
  run::Answerer,
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  store::TaskStore,
  task::Task,
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

  /// A message `text` of the person's in conversation `chat`, whose turn thinks now: the turns before it ended.
  fn says(&self, chat: ChatId, text: &str) {
    let mut db = self.db();
    self.end_turns(&mut db, chat);
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

  fn end_turns(&self, db: &mut ProjectDb, chat: ChatId) {
    for entry in db.chat_entries(chat).expect("entries") {
      if let EntryKind::Person { message, .. } = entry.kind {
        db.end_chat_turn(message, &TurnState::Answered, Timestamp::now())
          .expect("ended");
      }
    }
  }

  /// A wait of a run of task `task` starts a turn of the conversation by itself, which thinks now.
  fn wait_starts_a_turn(&self, task: &str) {
    let mut db = self.db();
    self.end_turns(&mut db, self.scope.chat);
    let chat = db.chat(self.scope.chat).expect("read").expect("kept");
    let wait = AgentWait {
      kind: WaitKind::Message,
      task: task.to_owned(),
      title: "Card form".to_owned(),
      run: RunId::generate(),
      number: 1,
      step: Some("Implement".to_owned()),
      agent: Some("Builder".to_owned()),
      place: None,
      others: 0,
      link: format!("/project/p/tasks/{task}/conversation"),
      own: false,
      automatic: true,
      continuing: None,
      merging: None,
    };
    db.keep_agent_wait(&chat, ("message:created:1", &wait), Timestamp::now())
      .expect("kept");
    let seq = db.automatic_waits_without_turn().expect("waits")[0].seq;
    assert!(
      db.start_wait_turn((chat.id(), seq), ChatMessageId::generate())
        .expect("started")
    );
  }

  /// The chat's tool `name` with `arguments`, in `scope`, as its agent calls it.
  fn chat_in(&self, scope: &ChatScope, name: &str, arguments: Value) -> Value {
    let Value::Object(arguments) = arguments else {
      panic!("arguments are an object");
    };
    let translate = Translate::new(|message| message.key.to_owned());
    self
      .runtime
      .block_on(tools::call(
        (&self.services, &translate, &self.runners),
        scope,
        name,
        arguments,
      ))
      .unwrap_or_else(|error| panic!("{name} refused: {}", error.to_json(|message| message.key.to_owned())))
  }

  fn chat(&self, name: &str, arguments: Value) -> Value {
    self.chat_in(&self.scope, name, arguments)
  }

  fn module(&self, name: &str) -> ModuleId {
    let made = self.chat("create_module", json!({ "project": "Kontur", "name": name }));
    made["module"]["id"]
      .as_str()
      .expect("an id")
      .parse()
      .expect("a module id")
  }

  /// `chat` subscribes to `target` with `order`.
  fn watches(&self, chat: ChatId, target: WatchTarget, order: &str) {
    let started = self
      .db()
      .start_subscription(&Subscription::new(chat, target, order, Timestamp::now()))
      .expect("kept");
    assert_eq!(started, WatchStart::Started);
  }

  /// `create_task` titled `title`, in `module` (by name) or in none: its key and the chat's answer.
  fn creates(&self, scope: &ChatScope, title: &str, module: Option<&str>) -> (String, Value) {
    let mut arguments = json!({ "project": "Kontur", "title": title });
    if let Some(module) = module {
      arguments["module"] = json!(module);
    }
    let told = self.chat_in(scope, "create_task", arguments);
    let key = told["task"]["key"].as_str().expect("a key").to_owned();
    (key, told)
  }

  /// The task `key`.
  fn task(&self, key: &str) -> Task {
    self
      .db()
      .tasks()
      .expect("tasks")
      .into_iter()
      .find(|task| task.key().to_string() == key)
      .expect("the task")
  }

  /// The subscription of the conversation to task `key`, if it has one.
  fn task_watch(&self, key: &str) -> Option<Subscription> {
    let task = self.task(key);
    self
      .db()
      .chat_subscriptions(self.scope.chat)
      .expect("read")
      .into_iter()
      .find(|watch| watch.target() == WatchTarget::Task(task.id()))
  }

  fn last_action(&self) -> ChatAction {
    self
      .db()
      .chat_entries(self.scope.chat)
      .expect("entries")
      .into_iter()
      .rev()
      .find_map(|entry| match entry.kind {
        EntryKind::Action(action) => Some(action),
        _ => None,
      })
      .expect("an action is kept")
  }
}

#[test]
fn created_task_outside_the_targets_in_a_persons_turn_is_watched_here_with_the_default_order() {
  let desktop = Desktop::new();
  let delivery = desktop.module("Delivery");
  desktop.watches(desktop.scope.chat, WatchTarget::Module(delivery), "Merge when green");
  let (key, told) = desktop.creates(&desktop.scope, "Shared rate limiter", None);
  assert_eq!(told["watch"]["kind"], "watched_here", "{told}");
  let watch = desktop.task_watch(&key).expect("a task subscription");
  assert_eq!(watch.order(), None, "a person's turn gives the default order");
  let action = desktop.last_action();
  assert_eq!(action.task.as_deref(), Some(key.as_str()));
  assert_eq!(action.watch, Some(CreatedWatch::WatchedHere));
}

#[test]
fn created_task_in_an_event_turn_takes_the_order_of_the_subscription_the_event_came_through() {
  let desktop = Desktop::new();
  let delivery = desktop.module("Delivery");
  let (card, _) = desktop.creates(&desktop.scope, "Card form", Some("Delivery"));
  assert_eq!(desktop.task(&card).module(), Some(delivery));
  desktop.watches(desktop.scope.chat, WatchTarget::Module(delivery), "Merge when green");
  desktop.watches(desktop.scope.chat, WatchTarget::WithoutModule, "Only report");
  desktop.module("Billing");
  desktop.wait_starts_a_turn(&card);
  let on_its_own = ChatScope {
    on_its_own: true,
    ..desktop.scope.clone()
  };
  let (key, told) = desktop.creates(&on_its_own, "Receipts", Some("Billing"));
  assert_eq!(told["watch"]["kind"], "watched_here", "{told}");
  let watch = desktop.task_watch(&key).expect("a task subscription");
  assert_eq!(watch.order(), Some("Merge when green"));
  assert_eq!(desktop.last_action().watch, Some(CreatedWatch::WatchedHere));
}

#[test]
fn created_task_whose_module_another_conversation_leads_is_not_taken_and_says_who_leads_it() {
  let desktop = Desktop::new();
  let delivery = desktop.module("Delivery");
  let billing = desktop.module("Billing");
  let other = ChatId::generate();
  desktop.says(other, "Billing work");
  desktop.watches(other, WatchTarget::Module(billing), "");
  desktop.watches(desktop.scope.chat, WatchTarget::Module(delivery), "");
  desktop.says(desktop.scope.chat, "Draft the receipts task");
  let (key, told) = desktop.creates(&desktop.scope, "Receipts", Some("Billing"));
  assert_eq!(told["watch"]["kind"], "led_elsewhere", "{told}");
  assert_eq!(told["watch"]["title"], "Billing work", "{told}");
  assert_eq!(desktop.task_watch(&key), None, "nothing is added");
  assert_eq!(
    desktop.last_action().watch,
    Some(CreatedWatch::LedElsewhere {
      chat: other,
      title: "Billing work".to_owned()
    })
  );
}

#[test]
fn created_task_inside_a_target_or_by_a_conversation_that_watches_nothing_adds_nothing() {
  let desktop = Desktop::new();
  let (key, told) = desktop.creates(&desktop.scope, "Receipts", None);
  assert!(told.get("watch").is_none(), "{told}");
  assert_eq!(desktop.task_watch(&key), None, "it watches nothing");
  assert_eq!(desktop.last_action().watch, None);

  let delivery = desktop.module("Delivery");
  desktop.watches(desktop.scope.chat, WatchTarget::Module(delivery), "");
  let (key, told) = desktop.creates(&desktop.scope, "Card form", Some("Delivery"));
  assert!(told.get("watch").is_none(), "{told}");
  assert_eq!(desktop.task_watch(&key), None, "its module's subscription takes it in");
  assert_eq!(desktop.last_action().watch, None);
}
