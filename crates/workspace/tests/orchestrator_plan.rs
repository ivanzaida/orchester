//! The Orchestrator's plan-only mode (ORC-646): in a turn that plans every tool that changes something is refused at
//! the tool layer (`plan_only`) and the refusal kept, while the tools that read still answer; the answer is a plan
//! (`propose_plan`), which works only in such a turn and replaces the plan that waited. Approving a plan keeps the
//! message whose ordinary turn carries it out, which acts but approves no card; the switch is the conversation's and
//! survives a restart.

use std::{fs, sync::Arc};

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, ProjectId},
  name::Name,
  orchestrator::{ActionOutcome, Chat, EntryKind, PlanState, TurnMode, TurnState},
  project::{Project, ProjectKey},
  run::Answerer,
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  store::TaskStore,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::{
  activity::Activity,
  app_mcp::{self, ToolError, Translate},
  orchestrator::{
    plan::{self, PROPOSE_PLAN},
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
    Self {
      services,
      runners,
      runtime,
      repo,
      scope,
      _data: data,
    }
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(self.repo.path()).expect("project database")
  }

  /// A message of the conversation, `text`, whose turn goes as `mode` says and thinks now: the turns before it ended.
  fn says(&self, text: &str, mode: TurnMode) -> ChatMessageId {
    let mut db = self.db();
    self.end_turns(&mut db);
    let body = MessageBody::new(text).expect("text");
    let chat = db
      .chat(self.scope.chat)
      .expect("read")
      .unwrap_or_else(|| Chat::start(self.scope.chat, &body, Timestamp::now()));
    let message = ChatMessageId::generate();
    db.keep_chat_message_in(
      &chat,
      (message, &body),
      (&[], Answerer::Person),
      (&TurnState::Thinking, Timestamp::now()),
      mode,
    )
    .expect("kept");
    message
  }

  /// A message of the conversation, `text`, sent as `mode` while the turn under way thinks, which reaches that turn as
  /// `state` says (steering, or joined): the turns before it go on.
  fn hands(&self, text: &str, mode: TurnMode, state: &TurnState) {
    let mut db = self.db();
    let body = MessageBody::new(text).expect("text");
    let chat = db.chat(self.scope.chat).expect("read").expect("kept");
    db.keep_chat_message_in(
      &chat,
      (ChatMessageId::generate(), &body),
      (&[], Answerer::Person),
      (state, Timestamp::now()),
      mode,
    )
    .expect("kept");
  }

  fn end_turns(&self, db: &mut ProjectDb) {
    for entry in db.chat_entries(self.scope.chat).expect("entries") {
      if let EntryKind::Person { message, .. } = entry.kind {
        db.end_chat_turn(message, &TurnState::Answered, Timestamp::now())
          .expect("ended");
      }
    }
  }

  /// The chat's tool `name` with `arguments`, as its agent calls it.
  fn chat(&self, name: &str, arguments: Value) -> Result<Value, ToolError> {
    let Value::Object(arguments) = arguments else {
      panic!("arguments are an object");
    };
    let translate = Translate::new(|message| message.key.to_owned());
    self.runtime.block_on(tools::call(
      (&self.services, &translate, &self.runners),
      &self.scope,
      name,
      arguments,
    ))
  }

  /// The plans of the conversation, by entry.
  fn plans(&self) -> Vec<(i64, orchester_core::orchestrator::ChatPlan)> {
    self
      .db()
      .chat_entries(self.scope.chat)
      .expect("entries")
      .into_iter()
      .filter_map(|entry| match entry.kind {
        EntryKind::Plan(plan) => Some((entry.seq, *plan)),
        _ => None,
      })
      .collect()
  }

  /// The codes of the refused actions the conversation keeps, by tool.
  fn refusals(&self) -> Vec<(String, String)> {
    self
      .db()
      .chat_entries(self.scope.chat)
      .expect("entries")
      .into_iter()
      .filter_map(|entry| match entry.kind {
        EntryKind::Action(action) => match action.outcome {
          ActionOutcome::Refused { code, .. } => Some((action.tool, code)),
          _ => None,
        },
        _ => None,
      })
      .collect()
  }
}

fn code(error: &ToolError) -> String {
  error.to_json(|message| message.key.to_owned())["error"]["code"]
    .as_str()
    .unwrap_or_default()
    .to_owned()
}

fn plan_arguments(action: &str) -> Value {
  json!({
    "project": "Kontur",
    "steps": [
      { "kind": "run", "action": action, "target": "KON-12 · from its last checkpoint" },
      { "kind": "draft", "action": "Create a draft", "target": "Per-endpoint retry settings" },
    ],
    "left_out": "Run #11: its branch conflicts with main",
    "uses": "1 run and 1 draft",
  })
}

#[test]
fn plan_turn_refuses_every_tool_that_changes_something_and_keeps_each_refusal() {
  let desktop = Desktop::new();
  desktop.says("Continue the runs and draft the follow-up", TurnMode::Plans);
  let writes: Vec<String> = tools::listed()
    .into_iter()
    .filter(|tool| tool.annotations.as_ref().and_then(|hints| hints.read_only_hint) != Some(true))
    .map(|tool| tool.name.to_string())
    .filter(|name| name != PROPOSE_PLAN)
    .collect();
  assert!(writes.len() > 30, "the chat has its write tools: {writes:?}");
  for name in &writes {
    let refused = desktop
      .chat(name, json!({ "project": "Kontur" }))
      .expect_err(&format!("{name} is refused in a turn that plans"));
    assert_eq!(code(&refused), "plan_only", "{name}");
  }
  let kept = desktop.refusals();
  assert_eq!(kept.len(), writes.len(), "each refusal is kept: {kept:?}");
  assert!(kept.iter().all(|(_, code)| code == "plan_only"));
  assert!(writes.iter().all(|name| kept.iter().any(|(tool, _)| tool == name)));
  assert!(desktop.db().tasks().expect("tasks").is_empty(), "nothing was created");
}

#[test]
fn plan_turn_still_reads() {
  let desktop = Desktop::new();
  desktop.says("What would you do next?", TurnMode::Plans);
  desktop
    .chat("list_tasks", json!({ "project": "Kontur" }))
    .expect("a read answers in a turn that plans");
  assert!(desktop.refusals().is_empty());
}

#[test]
fn ordinary_turn_acts_as_before() {
  let desktop = Desktop::new();
  desktop.says("Draft a task", TurnMode::Acts);
  desktop
    .chat(
      "create_task",
      json!({ "project": "Kontur", "title": "Per-endpoint retry settings" }),
    )
    .expect("an ordinary turn creates the draft");
  assert_eq!(desktop.db().tasks().expect("tasks").len(), 1);
}

#[test]
fn plan_turn_answers_with_a_plan_that_replaces_the_one_that_waited() {
  let desktop = Desktop::new();
  desktop.says("Continue the runs", TurnMode::Plans);
  let told = desktop
    .chat(PROPOSE_PLAN, plan_arguments("Continue run #8"))
    .expect("the plan is kept");
  assert_eq!(told["steps"], 2);
  let plans = desktop.plans();
  assert_eq!(plans.len(), 1);
  let plan = &plans[0].1;
  assert!(plan.waits());
  assert_eq!(plan.steps()[0].action(), "Continue run #8");
  assert_eq!(plan.left_out(), Some("Run #11: its branch conflicts with main"));
  desktop.says("Leave run #12 for now", TurnMode::Plans);
  desktop
    .chat(PROPOSE_PLAN, plan_arguments("Continue run #8 only"))
    .expect("the changed plan is kept");
  let plans = desktop.plans();
  assert!(matches!(plans[0].1.state(), PlanState::Replaced { .. }), "{plans:?}");
  assert!(plans[1].1.waits());
}

#[test]
fn propose_plan_is_refused_outside_a_turn_that_plans_and_for_an_empty_plan() {
  let desktop = Desktop::new();
  desktop.says("Continue the runs", TurnMode::Acts);
  let refused = desktop
    .chat(PROPOSE_PLAN, plan_arguments("Continue run #8"))
    .expect_err("an ordinary turn has no plan to propose");
  assert_eq!(code(&refused), "not_plan_only");
  desktop.says("Plan it", TurnMode::Plans);
  let empty = desktop
    .chat(
      PROPOSE_PLAN,
      json!({ "project": "Kontur", "steps": [{ "kind": "other", "action": "  " }] }),
    )
    .expect_err("a plan says what will happen");
  assert_eq!(code(&empty), "invalid_plan");
  assert!(desktop.plans().is_empty());
}

#[test]
fn a_turn_that_plans_tells_the_agent_so() {
  let desktop = Desktop::new();
  let message = desktop.says("Continue the runs", TurnMode::Plans);
  let turn = desktop.db().chat_turn(message).expect("read").expect("kept");
  assert!(turn.text.as_str().starts_with("[Plan only]"), "{}", turn.text.as_str());
  assert!(turn.text.as_str().ends_with("Continue the runs"));
  let ordinary = desktop.says("Now do it", TurnMode::Acts);
  let turn = desktop.db().chat_turn(ordinary).expect("read").expect("kept");
  assert_eq!(turn.text.as_str(), "Now do it");
}

#[test]
fn approving_a_plan_carries_it_out_as_an_ordinary_turn_that_approves_no_card() {
  let desktop = Desktop::new();
  desktop.says("Draft the follow-up", TurnMode::Plans);
  desktop
    .chat(PROPOSE_PLAN, plan_arguments("Continue run #8"))
    .expect("the plan is kept");
  let seq = desktop.plans()[0].0;
  let mut db = desktop.db();
  desktop.end_turns(&mut db);
  let carrying = ChatMessageId::generate();
  let approved = db
    .approve_chat_plan(seq, (carrying, Answerer::Person), Timestamp::now())
    .expect("approved")
    .expect("it waited");
  assert_eq!(approved.chat, desktop.scope.chat);
  assert!(matches!(desktop.plans()[0].1.state(), PlanState::Approved { .. }));
  let turn = db.chat_turn(carrying).expect("read").expect("kept");
  assert!(turn.turn.is_thinking());
  assert!(
    turn
      .text
      .as_str()
      .contains("1. Continue run #8 (KON-12 · from its last checkpoint)")
  );
  assert!(turn.text.as_str().contains("Left out: Run #11"));
  let entries = db.chat_entries(desktop.scope.chat).expect("entries");
  assert!(entries.iter().any(|entry| matches!(
    &entry.kind,
    EntryKind::Person { message, mode: TurnMode::CarriesOut, .. } if *message == carrying
  )));
  desktop
    .chat(
      "create_task",
      json!({ "project": "Kontur", "title": "Per-endpoint retry settings" }),
    )
    .expect("the approved plan acts");
  let refused = desktop
    .chat("apply_proposal", json!({ "project": "Kontur" }))
    .expect_err("an approved plan approves no card");
  assert_eq!(code(&refused), "approved_plan");
  assert!(
    db.approve_chat_plan(seq, (ChatMessageId::generate(), Answerer::Person), Timestamp::now())
      .expect("read")
      .is_none(),
    "a plan is approved once"
  );
}

#[test]
fn an_approval_the_runner_did_not_take_is_taken_back_and_a_dismissed_plan_waits_no_more() {
  let desktop = Desktop::new();
  desktop.says("Draft the follow-up", TurnMode::Plans);
  desktop
    .chat(PROPOSE_PLAN, plan_arguments("Continue run #8"))
    .expect("the plan is kept");
  let seq = desktop.plans()[0].0;
  let mut db = desktop.db();
  desktop.end_turns(&mut db);
  let carrying = ChatMessageId::generate();
  db.approve_chat_plan(seq, (carrying, Answerer::Person), Timestamp::now())
    .expect("approved");
  assert!(db.reopen_chat_plan(seq, carrying).expect("taken back"));
  assert!(desktop.plans()[0].1.waits());
  assert!(db.chat_turn(carrying).expect("read").is_none(), "its message is gone");
  plan::dismiss_in(desktop.repo.path(), seq).expect("dismissed");
  assert!(matches!(desktop.plans()[0].1.state(), PlanState::Dismissed { .. }));
  assert_eq!(plan::dismiss_in(desktop.repo.path(), seq), Err(plan::taken()));
}

#[test]
fn the_switch_is_the_conversations_and_survives_a_restart() {
  let desktop = Desktop::new();
  let mut db = desktop.db();
  let body = MessageBody::new("Plan the webhook retry work").expect("text");
  let chat = Chat::start(desktop.scope.chat, &body, Timestamp::now()).with_plan_only(true);
  db.keep_chat_message_in(
    &chat,
    (ChatMessageId::generate(), &body),
    (&[], Answerer::Person),
    (&TurnState::Thinking, Timestamp::now()),
    TurnMode::Plans,
  )
  .expect("kept");
  drop(db);
  let reopened = desktop.db().chat(desktop.scope.chat).expect("read").expect("kept");
  assert!(
    reopened.plan_only(),
    "a conversation started with the switch on keeps it"
  );
  plan::set_plan_only_in(desktop.repo.path(), desktop.scope.chat, false).expect("switched off");
  assert!(
    !desktop
      .db()
      .chat(desktop.scope.chat)
      .expect("read")
      .expect("kept")
      .plan_only()
  );
  plan::set_plan_only_in(desktop.repo.path(), desktop.scope.chat, true).expect("switched on");
  let listed = desktop.db().chats().expect("history");
  assert!(listed[0].chat.plan_only());
  let other = ChatId::generate();
  assert!(
    plan::set_plan_only_in(desktop.repo.path(), other, true).is_err(),
    "no such conversation"
  );
}

#[test]
fn a_plan_only_message_sent_now_into_a_turn_that_acts_refuses_its_changes() {
  for turn in [TurnMode::Acts, TurnMode::CarriesOut] {
    for state in [TurnState::Steering, TurnState::Joined] {
      let desktop = Desktop::new();
      desktop.says("Carry on with the runs", turn);
      desktop
        .chat("create_task", json!({ "project": "Kontur", "title": "Before" }))
        .expect("the turn acts before the message reaches it");
      desktop.hands("Also draft the follow-up", TurnMode::Plans, &state);
      let refused = desktop
        .chat("create_task", json!({ "project": "Kontur", "title": "After" }))
        .expect_err(&format!(
          "a {turn:?} turn that took a plan-only message as {state:?} plans"
        ));
      assert_eq!(code(&refused), "plan_only", "{turn:?} {state:?}");
      assert_eq!(desktop.refusals(), [("create_task".to_owned(), "plan_only".to_owned())]);
      assert_eq!(desktop.db().tasks().expect("tasks").len(), 1, "only the first draft");
      desktop
        .chat(PROPOSE_PLAN, plan_arguments("Continue run #8"))
        .expect("it answers the message with a plan");
    }
  }
}

#[test]
fn an_ordinary_message_sent_now_into_a_turn_that_acts_still_acts() {
  let desktop = Desktop::new();
  desktop.says("Carry on with the runs", TurnMode::Acts);
  desktop.hands("Also draft the follow-up", TurnMode::Acts, &TurnState::Joined);
  desktop
    .chat("create_task", json!({ "project": "Kontur", "title": "Follow-up" }))
    .expect("nothing asked for a plan");
  // A plan-only message an earlier turn took has no say in the next one.
  let desktop = Desktop::new();
  desktop.says("Plan it", TurnMode::Acts);
  desktop.hands("Only plan this", TurnMode::Plans, &TurnState::Joined);
  desktop.says("Now draft it", TurnMode::Acts);
  desktop
    .chat("create_task", json!({ "project": "Kontur", "title": "Follow-up" }))
    .expect("the turn under way acts");
}

#[test]
fn a_plan_is_not_approved_while_its_turn_still_thinks_nor_lets_that_turn_act() {
  let desktop = Desktop::new();
  desktop.says("Draft the follow-up", TurnMode::Plans);
  desktop
    .chat(PROPOSE_PLAN, plan_arguments("Continue run #8"))
    .expect("the plan is kept");
  let seq = desktop.plans()[0].0;
  let carrying = ChatMessageId::generate();
  assert_eq!(
    plan::approve_in(desktop.repo.path(), seq, (carrying, Answerer::Person)),
    Err(plan::busy()),
    "the turn that proposed it still thinks"
  );
  assert!(desktop.plans()[0].1.waits());
  assert!(desktop.db().chat_turn(carrying).expect("read").is_none());
  // Approved meanwhile by another way in, the turn that planned still changes nothing.
  desktop
    .db()
    .approve_chat_plan(seq, (carrying, Answerer::Person), Timestamp::now())
    .expect("approved")
    .expect("it waited");
  let refused = desktop
    .chat(
      "create_task",
      json!({ "project": "Kontur", "title": "Per-endpoint retry settings" }),
    )
    .expect_err("the planning turn still thinks");
  assert_eq!(code(&refused), "plan_only");
  assert!(desktop.db().tasks().expect("tasks").is_empty());
  // Once it ended, the plan is approved, and its turn acts.
  let desktop = Desktop::new();
  desktop.says("Draft the follow-up", TurnMode::Plans);
  desktop
    .chat(PROPOSE_PLAN, plan_arguments("Continue run #8"))
    .expect("the plan is kept");
  let seq = desktop.plans()[0].0;
  let mut db = desktop.db();
  desktop.end_turns(&mut db);
  plan::approve_in(desktop.repo.path(), seq, (carrying, Answerer::Person)).expect("approved");
  desktop
    .chat(
      "create_task",
      json!({ "project": "Kontur", "title": "Per-endpoint retry settings" }),
    )
    .expect("the approved plan acts");
}
