//! The chat's `watch` and `unwatch` (ORC-893, `orchestrator::watch::tool`): asked in words, the Orchestrator
//! subscribes and unsubscribes its own conversation through the same watch and stop as the commands, and the
//! conversation keeps only the line, at the person's request, with the order their words gave; a target another
//! conversation leads is not taken, but kept for its card; both tools are refused in a turn Orchester started.
//! `cargo test -p orchester-workspace --test orchestrator_watch_tools watch_tool`.

use std::{fs, sync::Arc};

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, ModuleId, ProjectId},
  name::Name,
  orchestrator::{
    Chat, ChatEvent, EntryKind, TurnMode, TurnState, WatchTarget,
    watch::{AtRequest, WatchLine},
  },
  project::{Project, ProjectKey},
  run::Answerer,
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::{
  activity::Activity,
  app_mcp::{self, ToolError, Translate},
  orchestrator::tools::{self, CHAT_TOOLS, ChatScope},
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

/// The app's session with one local project, Kontur, and a conversation of its chat whose person's turn thinks.
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
    desktop.says(desktop.scope.chat, "Watch the docs work");
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

  /// The chat's tool `name` with `arguments`, in `scope`, as its agent calls it.
  fn call_in(&self, scope: &ChatScope, name: &str, arguments: Value) -> Result<Value, ToolError> {
    let Value::Object(arguments) = arguments else {
      panic!("arguments are an object");
    };
    let translate = Translate::new(|message| message.key.to_owned());
    self.runtime.block_on(tools::call(
      (&self.services, &translate, &self.runners),
      scope,
      name,
      arguments,
    ))
  }

  fn call(&self, name: &str, arguments: Value) -> Value {
    self
      .call_in(&self.scope, name, arguments)
      .unwrap_or_else(|error| panic!("{name} refused: {}", error.to_json(|message| message.key.to_owned())))
  }

  fn module(&self, name: &str) -> ModuleId {
    let made = self.call("create_module", json!({ "project": "Kontur", "name": name }));
    made["module"]["id"]
      .as_str()
      .expect("an id")
      .parse()
      .expect("a module id")
  }

  /// A task titled `title`, by its key.
  fn task(&self, title: &str) -> String {
    let made = self.call("create_task", json!({ "project": "Kontur", "title": title }));
    made["task"]["key"].as_str().expect("a key").to_owned()
  }

  /// Who leads `target`, and with what order.
  fn leader(&self, target: WatchTarget) -> Option<(ChatId, String)> {
    self
      .db()
      .active_subscriptions()
      .expect("subscriptions")
      .of(target)
      .map(|led| (led.chat(), led.order().unwrap_or_default().to_owned()))
  }

  /// The watch entries of `chat`, in order: commands and lines.
  fn watch_entries(&self, chat: ChatId) -> Vec<ChatEvent> {
    self
      .db()
      .chat_entries(chat)
      .expect("entries")
      .into_iter()
      .filter_map(|entry| match entry.kind {
        EntryKind::Event(event @ (ChatEvent::WatchCommand { .. } | ChatEvent::Watch { .. })) => Some(event),
        _ => None,
      })
      .collect()
  }
}

fn code(refused: Result<Value, ToolError>) -> String {
  refused.expect_err("refused").code().to_owned()
}

#[test]
fn watch_tool_and_unwatch_tool_are_the_chats_own() {
  assert!(CHAT_TOOLS.contains(&"watch") && CHAT_TOOLS.contains(&"unwatch"));
  for drives in [false, true] {
    let listed = tools::listed(drives);
    for name in ["watch", "unwatch"] {
      let tool = listed.iter().find(|tool| tool.name == name).expect("listed");
      let told = tool.description.as_deref().unwrap_or_default();
      assert!(told.contains("never on your own initiative"), "{name}: {told}");
      assert!(told.contains("event_turn"), "{name}: {told}");
    }
  }
}

#[test]
fn watch_tool_subscribes_this_conversation_at_the_persons_request_with_the_order() {
  let desktop = Desktop::new();
  let docs = desktop.module("Docs");
  let key = desktop.task("Card form");

  let told = desktop.call(
    "watch",
    json!({ "project": "Kontur", "targets": ["docs", key], "order": "  Tell me when it merges. " }),
  );

  assert_eq!(told["watch"]["started"], json!(["Docs", key]), "{told}");
  assert_eq!(told["watch"]["order"], "Tell me when it merges.");
  let order = "Tell me when it merges.".to_owned();
  assert_eq!(
    desktop.leader(WatchTarget::Module(docs)),
    Some((desktop.scope.chat, order.clone()))
  );
  let entries = desktop.watch_entries(desktop.scope.chat);
  let [ChatEvent::Watch { line }] = entries.as_slice() else {
    panic!("only the line, no command of the person's: {entries:?}");
  };
  let WatchLine::Watching {
    started, at_request, ..
  } = line
  else {
    panic!("the subscription line: {line:?}");
  };
  assert_eq!(started.len(), 2);
  assert_eq!(at_request, &Some(AtRequest { order }), "at the person's request");
}

#[test]
fn watch_tool_does_not_take_a_target_another_conversation_leads() {
  let desktop = Desktop::new();
  let docs = WatchTarget::Module(desktop.module("Docs"));
  let theirs = ChatId::generate();
  desktop.says(theirs, "Docs and invoices");
  let their_scope = ChatScope {
    chat: theirs,
    ..desktop.scope.clone()
  };
  desktop
    .call_in(&their_scope, "watch", json!({ "targets": ["Docs"] }))
    .expect("theirs watches it");

  let told = desktop.call("watch", json!({ "targets": ["Docs"], "order": "Merge when green." }));

  assert_eq!(told["watch"]["started"], json!([]), "{told}");
  assert_eq!(told["watch"]["led_elsewhere"][0]["target"], "Docs");
  assert_eq!(told["watch"]["led_elsewhere"][0]["led_in"], "Docs and invoices");
  assert_eq!(
    desktop.leader(docs).map(|(chat, _)| chat),
    Some(theirs),
    "taking it over stays the person's"
  );
  let entries = desktop.watch_entries(desktop.scope.chat);
  let [
    ChatEvent::Watch {
      line: WatchLine::Watching {
        led_elsewhere,
        at_request,
        ..
      },
    },
  ] = entries.as_slice()
  else {
    panic!("the line that holds the card: {entries:?}");
  };
  assert_eq!(led_elsewhere.len(), 1, "the card's target");
  assert_eq!(led_elsewhere[0].chat, theirs);
  assert_eq!(
    at_request.as_ref().map(|asked| asked.order.as_str()),
    Some("Merge when green."),
    "the order Take it here gives the subscription"
  );
}

#[test]
fn watch_tool_unwatch_stops_at_the_persons_request_and_cancels_nothing() {
  let desktop = Desktop::new();
  let docs = WatchTarget::Module(desktop.module("Docs"));
  let key = desktop.task("Card form");
  desktop.call("watch", json!({ "targets": ["Docs", key, "without_module"] }));

  let told = desktop.call("unwatch", json!({ "targets": ["Docs"] }));

  assert_eq!(told["unwatch"]["stopped"], json!(["Docs"]), "{told}");
  assert_eq!(desktop.leader(docs), None);
  let Some(ChatEvent::Watch {
    line: WatchLine::Stopped {
      targets, at_request, ..
    },
  }) = desktop.watch_entries(desktop.scope.chat).pop()
  else {
    panic!("the stop line");
  };
  assert_eq!(targets.len(), 1);
  assert_eq!(at_request, Some(AtRequest::default()));
  let entries = desktop.watch_entries(desktop.scope.chat);
  assert!(
    !entries
      .iter()
      .any(|entry| matches!(entry, ChatEvent::WatchCommand { .. })),
    "no command of the person's: {entries:?}"
  );

  let all = desktop.call("unwatch", json!({ "all": true }));
  assert_eq!(all["unwatch"]["stopped"].as_array().map(Vec::len), Some(2), "{all}");
  assert_eq!(
    code(desktop.call_in(&desktop.scope, "unwatch", json!({ "all": true }))),
    "not_watched"
  );
}

#[test]
fn watch_tool_is_refused_in_a_turn_orchester_started() {
  let desktop = Desktop::new();
  let docs = WatchTarget::Module(desktop.module("Docs"));
  let event_turn = ChatScope {
    on_its_own: true,
    ..desktop.scope.clone()
  };

  for (name, arguments) in [
    ("watch", json!({ "targets": ["Docs"] })),
    ("unwatch", json!({ "all": true })),
  ] {
    assert_eq!(
      code(desktop.call_in(&event_turn, name, arguments)),
      "event_turn",
      "{name}"
    );
  }
  assert_eq!(desktop.leader(docs), None);
  assert!(desktop.watch_entries(desktop.scope.chat).is_empty(), "nothing kept");
}

#[test]
fn watch_tool_refuses_a_name_nothing_answers_to_and_another_project() {
  let desktop = Desktop::new();
  assert_eq!(
    code(desktop.call_in(&desktop.scope, "watch", json!({ "targets": ["Nowhere"] }))),
    "target_not_found"
  );
  assert_eq!(
    code(desktop.call_in(
      &desktop.scope,
      "watch",
      json!({ "project": "Elsewhere", "targets": ["project"] })
    )),
    "other_project"
  );
  assert!(desktop.watch_entries(desktop.scope.chat).is_empty());
}
