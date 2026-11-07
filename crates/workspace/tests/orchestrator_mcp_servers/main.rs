//! The Orchestrator chat's MCP server tools (ORC-89): adding a server, or a change that widens what agents may do, is
//! always a card with the whole server as applying leaves it, a secret by its name only, and only a person's *Apply*
//! carries it out, through the app's own tool; *Apply* is refused once the server, a secret it names or its program's
//! file changed since; a narrowing is made at once only in a turn the person asked for while they let automation
//! change agent permissions, and is a card otherwise. A plain variable's value and an endpoint's query values never
//! reach the card or what the agent is told.

use std::{fs, path::PathBuf, sync::Arc};

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, ProjectId, SecretId},
  name::Name,
  orchestrator::{
    ActionOutcome, Chat, ChatAction, EntryKind, TurnState,
    change::Change,
    mcp::{McpAuthorizationShown, McpChange, McpOperation, McpTransportShown, McpVariableShown},
  },
  project::{Project, ProjectKey},
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName, SecretValue, SecretVault, VaultError, VaultKey},
  store::{McpServerStore, SecretStore},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::{
  activity::Activity,
  app_mcp::{self, ToolError, Translate},
  orchestrator::{
    proposal::{Decision, decide},
    tools::{self, ChatScope},
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

/// No secret's value is read or kept here.
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

/// A plain value a person typed into a server's variable, which the chat must never be shown.
const PASTED: &str = "sk-live-pasted-by-a-person";
/// A token a person put in an endpoint's query.
const QUERY_TOKEN: &str = "query-token-abc123";

/// The app's session with one local project, Kontur, and a conversation of its Orchestrator chat.
struct Desktop {
  workspace: Workspace,
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
    let scope = conversation(&workspace, repo.path().to_path_buf());
    let desktop = Self {
      workspace,
      services,
      runners,
      runtime,
      repo,
      scope,
      _data: data,
    };
    desktop.add_secret("PENCIL_TOKEN");
    desktop
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(self.repo.path()).expect("project database")
  }

  /// A secret of the project, `name` in Development, as `NAME@development` names it.
  fn add_secret(&self, name: &str) {
    let secret = Secret::new(
      SecretId::generate(),
      NewSecret {
        name: SecretName::new(name).expect("a secret name"),
        kind: SecretKind::AccessToken,
        environment: Environment::Development,
      },
      Timestamp::now(),
    );
    self.db().create_secret(&secret).expect("the secret is stored");
  }

  fn let_automation_change_permissions(&self, allowed: bool) {
    self
      .workspace
      .set_automation_changes_permissions(allowed)
      .expect("a person sets it");
  }

  /// The chat's tool `name` with `arguments`, in `scope`, as its agent calls it.
  fn chat(&self, scope: &ChatScope, name: &str, arguments: Value) -> Result<Value, ToolError> {
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

  /// The app's own setup tool `name`, as another client of the app calls it.
  fn app(&self, name: &str, arguments: Value) -> Value {
    let Value::Object(arguments) = arguments else {
      panic!("arguments are an object");
    };
    let translate = Translate::new(|message| message.key.to_owned());
    self
      .runtime
      .block_on(app_mcp::setup::call(&self.services, &translate, name, arguments))
      .expect("a setup tool")
      .unwrap_or_else(|error| panic!("{name} refused: {}", error.to_json(|message| message.key.to_owned())))
  }

  /// The newest action of the conversation, and its entry's place.
  fn last_action(&self) -> (i64, ChatAction) {
    self
      .db()
      .chat_entries(self.scope.chat)
      .expect("entries")
      .into_iter()
      .rev()
      .find_map(|entry| match entry.kind {
        EntryKind::Action(action) => Some((entry.seq, action)),
        _ => None,
      })
      .expect("an action is kept")
  }

  /// The person's decision on the proposal at `seq`.
  fn decide(&self, seq: i64, decision: Decision) {
    let translate = Translate::new(|message| message.key.to_owned());
    self
      .runtime
      .block_on(decide(
        (&self.services, &translate, &self.runners),
        &self.scope,
        seq,
        decision,
      ))
      .expect("decided");
  }

  fn apply(&self, seq: i64) {
    self.decide(seq, Decision::Accept { widens: Vec::new() });
  }

  /// The project's servers by name, and whether each is on.
  fn servers(&self) -> Vec<(String, bool)> {
    self
      .db()
      .mcp_servers()
      .expect("servers")
      .into_iter()
      .map(|server| (server.name().as_str().to_owned(), server.enabled()))
      .collect()
  }
}

/// A conversation of the project's chat, its person's turn answered.
fn conversation(workspace: &Workspace, root: PathBuf) -> ChatScope {
  let mut db = ProjectDb::open(&root).expect("project database");
  let text = MessageBody::new("Can you add pencil mcp to the project?").expect("text");
  let chat = Chat::start(ChatId::generate(), &text, Timestamp::now());
  let message = ChatMessageId::generate();
  db.keep_chat_message(&chat, (message, &text), Timestamp::now())
    .expect("kept");
  db.end_chat_turn(message, &TurnState::Answered, Timestamp::now())
    .expect("answered");
  let project = workspace.projects().expect("projects")[0].id;
  ChatScope {
    project,
    name: "Kontur".to_owned(),
    root,
    chat: chat.id(),
    on_its_own: false,
  }
}

/// A program outside the project's repository, to stand for a server's: this test's own.
fn program() -> String {
  std::env::current_exe()
    .expect("the test program")
    .to_string_lossy()
    .into_owned()
}

fn pencil() -> Value {
  json!({
    "project": "Kontur",
    "name": "pencil",
    "program": program(),
    "args": ["--app", "desktop"],
    "environment": [
      { "name": "RUST_LOG", "value": "info" },
      { "name": "PENCIL_KEY", "value": "${secret:PENCIL_TOKEN@development}" },
    ],
    "exempt_tools": ["get_screenshot"],
  })
}

/// The MCP server change `action` keeps.
fn mcp_of(action: &ChatAction) -> &McpChange {
  match action.change.as_deref().map(|change| &change.change) {
    Some(Change::Mcp(mcp)) => mcp,
    other => panic!("an MCP server's change: {other:?}"),
  }
}

fn refusal(action: &ChatAction) -> &str {
  match &action.outcome {
    ActionOutcome::Refused { code, .. } => code,
    other => panic!("a refusal: {other:?}"),
  }
}

mod cards;
mod team;
