//! Approving the Orchestrator's proposal in the chat (ORC-96): in the turn the person's own message started, the
//! chat's `apply_proposal` applies a proposal that waited when they wrote, as the card's *Apply* does, and the card
//! keeps the message that approved it; a turn Orchester started by itself, a message automation sent, a proposal made
//! after the message (also while it was queued), one only a click applies and one that changed since are refused and
//! apply nothing; without a proposal it lists those that wait.

use std::{fs, sync::Arc};

use orchester_core::{
  conversation::MessageBody,
  id::{ChatMessageId, ProjectId},
  name::Name,
  orchestrator::{ActionOutcome, Chat, ChatAction, EntryKind, Queued, TurnState},
  project::{Project, ProjectKey},
  run::Answerer,
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  store::McpServerStore,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::{
  activity::Activity,
  app_mcp::{self, ToolError, Translate},
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

/// The app's session with one local project, Kontur, its MCP server `pencil`, and a conversation of its chat.
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
    let servers = Servers::new(workspace.clone(), Vault::with(Arc::new(NoVault)));
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
      chat: orchester_core::id::ChatId::generate(),
      on_its_own: false,
    };
    let desktop = Self {
      workspace,
      services,
      runners,
      runtime,
      repo,
      scope,
      _data: data,
    };
    desktop.let_automation_change_permissions(true);
    desktop.app(
      "add_mcp_server",
      json!({ "project": "Kontur", "name": "pencil", "program": program() }),
    );
    desktop.let_automation_change_permissions(false);
    desktop
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(self.repo.path()).expect("project database")
  }

  fn let_automation_change_permissions(&self, allowed: bool) {
    self
      .workspace
      .set_automation_changes_permissions(allowed)
      .expect("a person sets it");
  }

  /// A message of the conversation, `text` sent `by` the person or automation, whose turn thinks now: the turn before
  /// it ended.
  fn says(&self, text: &str, by: Answerer) -> ChatMessageId {
    let mut db = self.db();
    for entry in db.chat_entries(self.scope.chat).expect("entries") {
      if let EntryKind::Person { message, .. } = entry.kind {
        db.end_chat_turn(message, &TurnState::Answered, Timestamp::now())
          .expect("ended");
      }
    }
    let body = MessageBody::new(text).expect("text");
    let chat = db
      .chat(self.scope.chat)
      .expect("read")
      .unwrap_or_else(|| Chat::start(self.scope.chat, &body, Timestamp::now()));
    let message = ChatMessageId::generate();
    db.keep_chat_message_with(&chat, (message, &body), (&[], by), Timestamp::now())
      .expect("kept");
    message
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

  /// The chat proposes turning `pencil` off: a card, while the person does not let automation change permissions.
  fn propose_turning_off(&self) -> i64 {
    let told = self
      .chat(
        &self.scope,
        "mcp_set_server_enabled",
        json!({ "project": "Kontur", "server": "pencil", "enabled": false }),
      )
      .expect("proposed");
    assert_eq!(told["waits_for_person"], true, "{told}");
    self.last_action().0
  }

  fn action(&self, seq: i64) -> ChatAction {
    self
      .db()
      .chat_entries(self.scope.chat)
      .expect("entries")
      .into_iter()
      .find_map(|entry| match entry.kind {
        EntryKind::Action(action) if entry.seq == seq => Some(action),
        _ => None,
      })
      .expect("the action")
  }

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

  fn pencil_on(&self) -> bool {
    self
      .db()
      .mcp_servers()
      .expect("servers")
      .into_iter()
      .find(|server| server.name().as_str() == "pencil")
      .expect("pencil")
      .enabled()
  }

  fn apply(&self, scope: &ChatScope, seq: i64) -> Result<Value, ToolError> {
    self.chat(scope, "apply_proposal", json!({ "project": "Kontur", "proposal": seq }))
  }
}

/// A program outside the project's repository, to stand for a server's: this test's own.
fn program() -> String {
  std::env::current_exe()
    .expect("the test program")
    .to_string_lossy()
    .into_owned()
}

fn code(error: &ToolError) -> &str {
  match error {
    ToolError::Refused { code, .. } => code,
    other => panic!("a refusal: {other:?}"),
  }
}

#[test]
fn chat_approval_applies_the_proposal_the_persons_message_approves_and_the_card_keeps_it() {
  let desktop = Desktop::new();
  desktop.says("Can you turn pencil off?", Answerer::Person);
  let seq = desktop.propose_turning_off();
  let approved = desktop.says("yes, do it", Answerer::Person);
  let told = desktop.apply(&desktop.scope, seq).expect("applied");
  assert_eq!(told["applied"], true, "{told}");
  assert!(!desktop.pencil_on(), "the server is off");
  let action = desktop.action(seq);
  assert_eq!(action.outcome, ActionOutcome::Applied);
  let approval = action.approved_in.expect("the card keeps the message that approved it");
  assert_eq!(approval.message, approved);
  assert_eq!(approval.words, "yes, do it");
  assert!(action.settled.is_some() && action.claimed.is_none());
  let again = desktop.apply(&desktop.scope, seq).expect_err("applied once");
  assert_eq!(code(&again), "not_pending");
}

#[test]
fn chat_approval_is_refused_in_a_turn_orchester_started_and_for_automations_message() {
  let desktop = Desktop::new();
  desktop.says("Can you turn pencil off?", Answerer::Person);
  let seq = desktop.propose_turning_off();
  desktop.says("run it", Answerer::Person);
  let on_its_own = ChatScope {
    on_its_own: true,
    ..desktop.scope.clone()
  };
  let refused = desktop.apply(&on_its_own, seq).expect_err("nobody asked");
  assert_eq!(code(&refused), "not_persons_turn");
  desktop.says("apply", Answerer::Automation);
  let refused = desktop
    .apply(&desktop.scope, seq)
    .expect_err("automation's words approve nothing");
  assert_eq!(code(&refused), "automation_input");
  assert!(desktop.pencil_on(), "nothing was applied");
  let action = desktop.action(seq);
  assert!(action.waits() && action.approved_in.is_none(), "{action:?}");
}

#[test]
fn chat_approval_is_refused_for_a_proposal_made_after_the_persons_message() {
  let desktop = Desktop::new();
  desktop.says("Turn pencil off", Answerer::Person);
  let seq = desktop.propose_turning_off();
  let refused = desktop.apply(&desktop.scope, seq).expect_err("the person never saw it");
  assert_eq!(code(&refused), "not_pending");
  assert!(desktop.pencil_on());
  assert!(desktop.action(seq).waits());
}

#[test]
fn chat_approval_is_refused_for_a_proposal_made_while_the_persons_message_was_queued() {
  let desktop = Desktop::new();
  let asked = desktop.says("Turn pencil off", Answerer::Person);
  // Written while the answer was under way, so it waits for its turn (ORC-92), before the card was made.
  let mut db = desktop.db();
  let chat = db.chat(desktop.scope.chat).expect("read").expect("the chat");
  let written = Timestamp::from_unix_millis(Timestamp::now().unix_millis() - 1_000);
  let body = MessageBody::new("run it").expect("text");
  let queued = (&TurnState::Queued(Queued::AfterAnswer), written);
  db.keep_chat_message_as(
    &chat,
    (ChatMessageId::generate(), &body),
    (&[], Answerer::Person),
    queued,
  )
  .expect("queued");
  let seq = desktop.propose_turning_off();
  db.end_chat_turn(asked, &TurnState::Answered, Timestamp::now())
    .expect("ended");
  // Its turn starts now and moves to the conversation's end, after the card.
  assert!(db.start_queued_chat_turn().expect("started").is_some());
  let refused = desktop.apply(&desktop.scope, seq).expect_err("the person never saw it");
  assert_eq!(code(&refused), "not_pending");
  assert!(desktop.pencil_on() && desktop.action(seq).waits());
}

#[test]
fn chat_approval_is_refused_like_the_cards_apply_once_the_proposal_changed() {
  let desktop = Desktop::new();
  desktop.says("Can you turn pencil off?", Answerer::Person);
  let seq = desktop.propose_turning_off();
  desktop.let_automation_change_permissions(true);
  desktop.app(
    "update_mcp_server",
    json!({ "project": "Kontur", "server": "pencil", "args": ["--other"] }),
  );
  desktop.let_automation_change_permissions(false);
  desktop.says("run it", Answerer::Person);
  let refused = desktop.apply(&desktop.scope, seq).expect_err("stale");
  assert_eq!(code(&refused), "changed_since_proposed");
  assert!(desktop.pencil_on(), "nothing was applied");
  let action = desktop.action(seq);
  assert!(
    matches!(&action.outcome, ActionOutcome::Refused { code, .. } if code == "changed_since_proposed"),
    "{action:?}"
  );
  assert!(
    action.approved_in.is_some(),
    "the card says the person's approval was refused"
  );
}

#[test]
fn chat_approval_lists_what_waits_and_never_applies_what_only_a_click_applies() {
  let desktop = Desktop::new();
  desktop.says("Turn pencil off and add another server", Answerer::Person);
  let off = desktop.propose_turning_off();
  desktop
    .chat(
      &desktop.scope,
      "mcp_add_server",
      json!({ "project": "Kontur", "name": "figma", "program": program() }),
    )
    .expect("proposed");
  let add = desktop.last_action().0;
  desktop.says("apply", Answerer::Person);
  let listed = desktop
    .chat(&desktop.scope, "apply_proposal", json!({ "project": "Kontur" }))
    .expect("listed");
  assert_eq!(listed["applied"], false, "{listed}");
  let pending: Vec<(i64, bool)> = listed["pending"]
    .as_array()
    .expect("pending")
    .iter()
    .map(|item| {
      (
        item["proposal"].as_i64().expect("a number"),
        item["click_only"].as_bool().expect("click_only"),
      )
    })
    .collect();
  assert_eq!(pending, [(off, false), (add, true)], "{listed}");
  assert!(
    desktop.pencil_on() && desktop.action(off).waits(),
    "a list applies nothing"
  );
  let refused = desktop.apply(&desktop.scope, add).expect_err("adding a server widens");
  assert_eq!(code(&refused), "click_only");
  assert!(desktop.action(add).waits(), "it still waits for the person's click");
  assert_eq!(
    desktop.db().mcp_servers().expect("servers").len(),
    1,
    "nothing was added"
  );
}

#[test]
fn chat_approval_tool_says_when_it_applies_and_what_stays_a_click() {
  assert!(tools::CHAT_TOOLS.contains(&"apply_proposal"));
  let told = tools::listed()
    .into_iter()
    .find(|tool| tool.name == "apply_proposal")
    .and_then(|tool| tool.description.map(|told| told.into_owned()))
    .expect("apply_proposal is listed with a description");
  for part in [
    "ask the person which one; never guess",
    "make a new proposal instead",
    "not_persons_turn",
    "automation_input",
    "click_only",
    "changed_since_proposed",
  ] {
    assert!(told.contains(part), "apply_proposal says {part}: {told}");
  }
}
