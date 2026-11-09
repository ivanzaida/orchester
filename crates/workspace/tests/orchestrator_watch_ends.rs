//! Subscriptions that end by themselves (ORC-764, `orchestrator::watch_ends`): once every task of a watched module is
//! Done or Canceled, or a watched task is, the waits follower ends the subscription, keeps the line in its conversation
//! and, with *Let the Orchestrator answer agents* on and the Orchestrator ready, starts one turn that writes the
//! summary and changes nothing; reopening a task does not watch it again, and the tasks without a module and the whole
//! project never end by themselves.
//! `cargo test -p orchester-workspace --test orchestrator_watch_ends watch_ends`.

use std::{collections::HashSet, fs, sync::Arc};

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
  },
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, ConnectionId, ProjectId},
  name::Name,
  orchestrator::{
    Chat, EntryKind, OrchestratorSettings, Subscription, TurnMode, TurnState, WaitTurn, WatchTarget,
    watch::{EndReason, WatchEnd, WatchSummary},
  },
  project::{Project, ProjectKey},
  resource::ModelId,
  run::Answerer,
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  store::{ConnectionStore, TaskStore},
  task::{Task, organization::WorkflowStatus},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{AppDb, ProjectDb, WatchStart};
use orchester_workspace::{
  activity::Activity,
  app_mcp::{self, ToolError, Translate},
  orchestrator::{
    tools::{self, ChatScope},
    waits::{Followed, Switches, check_merges_in, follow_in},
    watch_ends::{ended_aside, ended_done, ended_line},
  },
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
    desktop.says("Look after the mobile release");
    desktop
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(self.repo.path()).expect("project database")
  }

  /// A message `text` of the person's in the conversation, whose turn thinks now.
  fn says(&self, text: &str) {
    let mut db = self.db();
    let body = MessageBody::new(text).expect("text");
    let chat = Chat::start(self.scope.chat, &body, Timestamp::now());
    db.keep_chat_message_in(
      &chat,
      (ChatMessageId::generate(), &body),
      (&[], Answerer::Person),
      (&TurnState::Thinking, Timestamp::now()),
      TurnMode::Acts,
    )
    .expect("kept");
  }

  /// The person's turns of the conversation end: only then does a turn start by itself in it.
  fn answered(&self) {
    let mut db = self.db();
    for entry in db.chat_entries(self.scope.chat).expect("entries") {
      if let EntryKind::Person { message, .. } = entry.kind {
        db.end_chat_turn(message, &TurnState::Answered, Timestamp::now())
          .expect("ended");
      }
    }
  }

  /// A checked, signed-in Claude Code connection, published as what the Orchestrator runs on.
  fn ready(&self) {
    let mut db = self.db();
    let mut connection = AgentConnection::new(
      ConnectionId::generate(),
      Name::new("Claude · Kontur").expect("name"),
      AgentProvider::ClaudeCodeCli,
      self.repo.path().join("claude.exe"),
      Timestamp::now(),
    )
    .expect("absolute program");
    let haiku = CatalogModel::new(ModelId::new("haiku").expect("model"), Vec::new());
    connection
      .record_check(ConnectionCheck {
        at: Timestamp::now(),
        result: CheckResult::Available {
          version: "2.1.89".into(),
          sign_in: SignIn::SignedIn,
          catalog: ModelCatalog::new(CatalogSource::Provider, vec![haiku]),
        },
      })
      .expect("check");
    db.create_connection(&connection).expect("connection");
    let settings = OrchestratorSettings::new(connection.id(), ModelId::new("haiku").expect("model"), None);
    db.publish_orchestrator(&settings, Timestamp::now()).expect("published");
  }

  /// The chat's tool `name` with `arguments`, in `scope`, as its agent calls it.
  fn call(&self, scope: &ChatScope, name: &str, arguments: Value) -> Result<Value, ToolError> {
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

  fn chat(&self, name: &str, arguments: Value) -> Value {
    self
      .call(&self.scope, name, arguments)
      .unwrap_or_else(|error| panic!("{name} refused: {}", error.to_json(|message| message.key.to_owned())))
  }

  /// A module named `name`, with a task for each of `titles`: their keys.
  fn module(&self, name: &str, titles: &[&str]) -> (WatchTarget, Vec<String>) {
    let made = self.chat("create_module", json!({ "project": "Kontur", "name": name }));
    let module = made["module"]["id"]
      .as_str()
      .expect("an id")
      .parse()
      .expect("a module id");
    let keys = titles.iter().map(|title| self.creates(title, Some(name))).collect();
    (WatchTarget::Module(module), keys)
  }

  /// `create_task` titled `title`, in `module` (by name) or in none: its key.
  fn creates(&self, title: &str, module: Option<&str>) -> String {
    let mut arguments = json!({ "project": "Kontur", "title": title });
    if let Some(module) = module {
      arguments["module"] = json!(module);
    }
    let told = self.chat("create_task", arguments);
    told["task"]["key"].as_str().expect("a key").to_owned()
  }

  /// Task `key` moves to workflow status `status` (`done`, `canceled`, `todo`).
  fn moves(&self, key: &str, status: &str) {
    self.chat(
      "update_task",
      json!({ "project": "Kontur", "key": key, "workflow_status": status }),
    );
  }

  fn task(&self, key: &str) -> Task {
    self
      .db()
      .tasks()
      .expect("tasks")
      .into_iter()
      .find(|task| task.key().to_string() == key)
      .expect("the task")
  }

  /// The conversation watches `target` with `order`.
  fn watches(&self, target: WatchTarget, order: &str) {
    let subscription = Subscription::new(self.scope.chat, target, order, Timestamp::now());
    let started = self.db().start_subscription(&subscription).expect("kept");
    assert_eq!(started, WatchStart::Started);
  }

  /// What the conversation watches now.
  fn watching(&self) -> Vec<WatchTarget> {
    self
      .db()
      .chat_subscriptions(self.scope.chat)
      .expect("read")
      .iter()
      .map(Subscription::target)
      .collect()
  }

  /// Follows the waits with the project's runner attached and *Let the Orchestrator answer agents* on.
  fn follow(&self) -> Followed {
    let (root, programs) = (self.repo.path(), Programs::processes());
    let checks = check_merges_in((root, &programs), &HashSet::new()).expect("checked");
    let switches = Switches { drives: true };
    follow_in((root, self.scope.project, &programs), &checks, (switches, true)).expect("followed")
  }

  /// The ends of subscriptions the conversation keeps, with their turns.
  fn ends(&self) -> Vec<(WatchSummary, Option<WaitTurn>)> {
    self
      .db()
      .chat_entries(self.scope.chat)
      .expect("entries")
      .into_iter()
      .filter_map(|entry| match entry.kind {
        EntryKind::WatchEnded { summary, turn } => Some((*summary, turn)),
        _ => None,
      })
      .collect()
  }
}

fn refusal(error: &ToolError) -> &str {
  match error {
    ToolError::Refused { code, .. } => code,
    other => panic!("a refusal: {other:?}"),
  }
}

#[test]
fn watch_ends_a_module_once_its_tasks_are_done_or_canceled_with_its_line_and_one_summary_turn() {
  let desktop = Desktop::new();
  desktop.ready();
  let (module, keys) = desktop.module("Mobile release", &["Card form", "Receipts"]);
  desktop.watches(module, "Merge when green");
  desktop.answered();
  desktop.moves(&keys[0], "done");
  assert_eq!(desktop.follow(), Followed::default(), "a task of it is still open");
  assert_eq!(desktop.watching(), [module]);

  desktop.moves(&keys[1], "canceled");
  let followed = desktop.follow();
  assert!(followed.kept, "{followed:?}");
  let turn = followed.turn.expect("the end starts its summary turn");
  assert_eq!(turn.chat, desktop.scope.chat, "in the conversation that watched it");
  assert!(desktop.watching().is_empty(), "the subscription ended");
  let history = desktop
    .db()
    .chat_subscription_history(desktop.scope.chat)
    .expect("history");
  let closed = EndReason::ModuleClosed { done: 1, canceled: 1 };
  assert_eq!(history[0].ended.map(|(_, why)| why), Some(WatchEnd::ByItself(closed)));

  let ends = desktop.ends();
  assert_eq!(ends.len(), 1, "{ends:?}");
  let (summary, started) = &ends[0];
  assert_eq!(
    (
      summary.name.as_str(),
      summary.reason,
      summary.order.as_deref(),
      summary.automatic
    ),
    ("Mobile release", closed, Some("Merge when green"), true)
  );
  assert_eq!(started.as_ref().map(|started| started.message), Some(turn.message));
  let line = ended_line(summary);
  assert_eq!(line.key, "orchestrator.watch.ended.moduleClosed");
  assert_eq!(line.args, [("module", "Mobile release".to_owned())]);
  assert_eq!(ended_aside(summary).key, "orchestrator.watch.ended.asideModule");
  assert!(!ended_done(summary.reason), "a Canceled task is no success");
  for (done, canceled, success) in [(2, 0, true), (0, 2, false), (0, 0, false)] {
    assert_eq!(
      ended_done(EndReason::ModuleClosed { done, canceled }),
      success,
      "{done} {canceled}"
    );
  }
  let prompt = desktop
    .db()
    .chat_turn(turn.message)
    .expect("read")
    .expect("the turn's prompt");
  let prompt = prompt.text.as_str();
  assert!(prompt.contains("the module Mobile release"), "{prompt}");
  assert!(prompt.contains("Change nothing"), "{prompt}");

  desktop
    .db()
    .end_chat_turn(turn.message, &TurnState::Answered, Timestamp::now())
    .expect("ended");
  assert_eq!(desktop.follow(), Followed::default(), "one line and one turn at most");
  assert_eq!(desktop.ends().len(), 1);
}

#[test]
fn watch_ends_the_summary_turn_changes_nothing() {
  let desktop = Desktop::new();
  desktop.ready();
  let (module, keys) = desktop.module("Mobile release", &["Card form"]);
  let other = desktop.creates("Receipts", None);
  desktop.watches(module, "");
  desktop.answered();
  desktop.moves(&keys[0], "done");
  desktop.follow().turn.expect("the summary turn thinks");
  let summary_turn = ChatScope {
    on_its_own: true,
    ..desktop.scope.clone()
  };
  let renamed = json!({ "project": "Kontur", "key": other, "title": "Renamed by a summary" });
  let refused = desktop
    .call(&summary_turn, "update_task", renamed)
    .expect_err("refused");
  assert_eq!(refusal(&refused), "report_turn");
  assert_eq!(
    desktop.task(&other).details().title.as_str(),
    "Receipts",
    "the task is as it was"
  );
  let held = desktop
    .call(&summary_turn, "hold_queue", json!({ "project": "Kontur" }))
    .expect_err("refused");
  assert_eq!(refusal(&held), "report_turn");
  assert!(!desktop.db().queue_holds().expect("holds").project, "nothing held");
}

#[test]
fn watch_ends_a_task_with_its_line_but_no_turn_while_the_orchestrator_cannot_answer() {
  let desktop = Desktop::new();
  let key = desktop.creates("Card form", None);
  desktop.watches(WatchTarget::Task(desktop.task(&key).id()), "");
  desktop.answered();
  desktop.moves(&key, "done");
  let followed = desktop.follow();
  assert!(followed.kept && followed.turn.is_none(), "{followed:?}");
  assert!(desktop.watching().is_empty());
  let ends = desktop.ends();
  let (summary, turn) = &ends[0];
  assert_eq!(
    (summary.reason, summary.automatic, turn),
    (EndReason::TaskDone, false, &None)
  );
  assert_eq!(summary.title.as_deref(), Some("Card form"));
  let line = ended_line(summary);
  assert_eq!(
    (line.key, line.args),
    ("orchestrator.watch.ended.taskDone", vec![("task", key)])
  );
  assert_eq!(
    ended_aside(summary).key,
    "orchestrator.watch.ended.aside",
    "no summary below"
  );
  assert!(ended_done(summary.reason), "a Done task is a success");

  // Once the Orchestrator is ready, an end kept without a turn starts none later.
  desktop.ready();
  assert_eq!(desktop.follow(), Followed::default());
}

#[test]
fn watch_ends_reopening_a_task_does_not_watch_it_again() {
  let desktop = Desktop::new();
  let key = desktop.creates("Card form", None);
  let target = WatchTarget::Task(desktop.task(&key).id());
  desktop.watches(target, "");
  desktop.answered();
  desktop.moves(&key, "canceled");
  assert!(desktop.follow().kept);
  assert_eq!(desktop.ends()[0].0.reason, EndReason::TaskCanceled);

  desktop.moves(&key, "todo");
  assert_eq!(desktop.task(&key).organization().status(), WorkflowStatus::Todo);
  assert_eq!(desktop.follow(), Followed::default());
  assert!(desktop.watching().is_empty(), "reopening does not subscribe it again");
  desktop.moves(&key, "done");
  assert_eq!(desktop.follow(), Followed::default());
  assert_eq!(desktop.ends().len(), 1, "no second line");
}

#[test]
fn watch_ends_never_for_the_tasks_without_a_module_or_the_whole_project() {
  let desktop = Desktop::new();
  desktop.ready();
  let key = desktop.creates("Card form", None);
  desktop.watches(WatchTarget::WithoutModule, "");
  desktop.watches(WatchTarget::Project, "");
  desktop.answered();
  desktop.moves(&key, "done");
  assert_eq!(desktop.follow(), Followed::default());
  assert_eq!(desktop.watching(), [WatchTarget::WithoutModule, WatchTarget::Project]);
  assert!(desktop.ends().is_empty());
}
