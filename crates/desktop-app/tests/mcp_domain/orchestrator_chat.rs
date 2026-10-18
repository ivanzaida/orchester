//! The Orchestrator chat follows and drives the agents' work (ORC-42), on the seeded Kontur: a step waiting for a
//! message shows what its agent wrote (`get_run`, `list_questions`), a task's conversation reads as its tab shows it
//! (`get_conversation`); the chat's `merge_run` and `send_message` are proposals until the person's switch lets the
//! chat do them, then done at once and kept as its actions, a message named the Orchestrator's in the task's
//! conversation; a message that would decide a plan is refused; and an opened project's waits are kept in the
//! Orchestrator's conversation, each with its card, starting no turn while no runner attaches.

use std::{path::PathBuf, thread, time::Duration};

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
  },
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, ConnectionId},
  name::Name,
  orchestrator::{
    ActionOutcome, Chat, ChatAction, EntryKind, OrchestratorSettings, TurnState, WaitKind, change::Change,
  },
  resource::ModelId,
  run::Run,
  store::{ConnectionStore, ConversationStore, MergeStore, RunStore, TaskStore},
  task::Task,
  time::Timestamp,
};
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::{
  app_mcp::{self, ToolError, Translate},
  orchestrator::tools::{self, ChatScope},
  workspace::OrchestratorConsent,
};
use serde_json::{Value, json};

use super::Served;

/// What the seed's agent asks in "Sync contacts with HubSpot" (`shapes::QUESTION`).
const ASKED: &str = "Should a contact that exists in both keep the HubSpot owner or ours?";
const ASKING: &str = "Sync contacts with HubSpot";
const READY: &str = "Show the signed-in account";
const PLANNING: &str = "Plan the GitHub app permissions";

/// The seeded task titled `title` and its latest run.
fn task_run(db: &ProjectDb, title: &str) -> (Task, Run) {
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.details().title.as_str() == title)
    .unwrap_or_else(|| panic!("the seeded task {title}"));
  let run = db
    .runs_of_task(task.id())
    .expect("runs")
    .pop()
    .expect("a run")
    .expect("a readable run");
  (task, run)
}

/// A conversation of the project's Orchestrator chat, its person's turn answered: what the chat's tools act in.
pub(super) fn chat(served: &Served, db: &mut ProjectDb) -> ChatScope {
  let text = MessageBody::new("Merge and answer for me").expect("text");
  let chat = Chat::start(ChatId::generate(), &text, Timestamp::now());
  let message = ChatMessageId::generate();
  db.keep_chat_message(&chat, (message, &text), Timestamp::now())
    .expect("kept");
  db.end_chat_turn(message, &TurnState::Answered, Timestamp::now())
    .expect("answered");
  let open = &served.services().workspace.projects().expect("projects")[0];
  ChatScope {
    project: open.id,
    name: "Kontur".to_owned(),
    root: open.repo_root.clone(),
    chat: chat.id(),
  }
}

/// Calls the chat's tool `name` with `arguments` in `scope` as its agent does, while the window runs.
pub(super) fn chat_call(
  served: &mut Served,
  scope: &ChatScope,
  name: &str,
  arguments: Value,
) -> Result<Value, ToolError> {
  let s = served.services();
  let services = app_mcp::Services::new(
    s.workspace.clone(),
    s.runners.clone(),
    s.servers.clone(),
    s.programs.clone(),
    (s.changes.clone(), s.activity.clone(), s.jobs.clone()),
  );
  let runners = s.runners.clone();
  let (scope, name) = (scope.clone(), name.to_owned());
  let Value::Object(arguments) = arguments else {
    panic!("arguments are an object")
  };
  let called = served._runtime.spawn(async move {
    let translate = Translate::new(|message| message.key.to_owned());
    tools::call((&services, &translate, &runners), &scope, &name, arguments).await
  });
  while !called.is_finished() {
    served.pass();
    thread::sleep(Duration::from_millis(5));
  }
  served._runtime.block_on(called).expect("the call ends")
}

/// The actions of `scope`'s conversation, in order.
fn actions(db: &ProjectDb, scope: &ChatScope) -> Vec<ChatAction> {
  db.chat_entries(scope.chat)
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Action(action) => Some(action),
      _ => None,
    })
    .collect()
}

fn allow(served: &Served, consent: OrchestratorConsent) {
  served
    .services()
    .workspace
    .set_orchestrator_allows(consent, true)
    .expect("the person turned it on");
}

#[test]
fn chat_reads_what_a_waiting_agent_wrote_and_the_tasks_conversation() {
  let (mut served, seeded) = Served::seeded("tasks");
  let db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let (task, run) = task_run(&db, ASKING);
  let key = task.key().to_string();
  let conversation = format!("/project/{}/tasks/{key}/conversation", seeded.project);
  let read = served.ok("get_run", &json!({ "project": "Kontur", "run": run.id().to_string() }));
  assert_eq!(read["awaiting"]["answers"], 1, "{read}");
  let waiting = &read["awaiting"]["messages"][0];
  assert_eq!(waiting["last_words"]["text"], ASKED, "{read}");
  assert_eq!(waiting["last_words"]["question"], true);
  assert_eq!(waiting["link"], conversation);

  let listed = served.ok("list_questions", &json!({ "project": "Kontur" }));
  let message = listed["waiting_for_message"]
    .as_array()
    .expect("the steps waiting for a message")
    .iter()
    .find(|wait| wait["task"] == key)
    .unwrap_or_else(|| panic!("{key} waits for a message: {listed}"))
    .clone();
  assert_eq!(message["last_words"]["text"], ASKED);
  assert_eq!(message["run"]["id"], run.id().to_string());

  let read = served.ok("get_conversation", &json!({ "project": "Kontur", "task": key }));
  let entries = read["entries"].as_array().expect("entries");
  let question = entries
    .iter()
    .find(|entry| entry["kind"] == "agent_question")
    .unwrap_or_else(|| panic!("the agent's question: {read}"));
  assert_eq!(question["text"], ASKED);
  assert_eq!(question["open"], true);
  assert_eq!(question["author"]["kind"], "agent");
  assert_eq!(question["run"]["number"], run.number().get());
  assert_eq!(question["link"], conversation);
  assert!(question["step"].is_string() && question["at"].is_i64(), "{question}");
  assert_eq!(entries.last(), Some(question), "newest last: {read}");

  let page = served.ok(
    "get_conversation",
    &json!({ "project": "Kontur", "task": key, "limit": 1 }),
  );
  assert_eq!(page["entries"].as_array().map(Vec::len), Some(1));
  assert_eq!(page["more"], true, "older entries are left for the next page");
  let cursor = page["entries"][0]["cursor"].as_str().expect("a cursor").to_owned();
  let older = served.ok(
    "get_conversation",
    &json!({ "project": "Kontur", "task": key, "before": cursor }),
  );
  assert!(
    older["entries"]
      .as_array()
      .expect("entries")
      .iter()
      .all(|entry| entry["cursor"] != cursor.as_str()),
    "before reads only what is older: {older}"
  );
}

#[test]
fn chat_merge_is_a_proposal_until_the_person_lets_the_orchestrator_merge() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let (_, run) = task_run(&db, READY);
  let scope = chat(&served, &mut db);
  let merge = json!({ "project": "Kontur", "run": run.id().to_string() });
  let told = chat_call(&mut served, &scope, "merge_run", merge.clone()).expect("proposed");
  assert_eq!(told["waits_for_person"], true, "{told}");
  assert!(
    db.run_merge(run.id()).expect("read").is_none(),
    "nothing merged on the agent's word"
  );
  assert_eq!(actions(&db, &scope)[0].outcome, ActionOutcome::Waits);

  allow(&served, OrchestratorConsent::MergeRuns);
  let merged = chat_call(&mut served, &scope, "merge_run", merge).expect("merged at once");
  assert_eq!(merged["merge"]["run"], run.id().to_string(), "{merged}");
  assert!(db.run_merge(run.id()).expect("read").is_some(), "the run is merged");
  let kept = actions(&db, &scope).remove(1);
  assert_eq!(kept.outcome, ActionOutcome::Done, "kept as the chat's own action");
  assert!(matches!(
    kept.change.map(|change| change.change),
    Some(Change::Merge { .. })
  ));
}

#[test]
fn chat_message_is_a_proposal_until_the_person_lets_the_orchestrator_answer() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let (task, run) = task_run(&db, ASKING);
  let scope = chat(&served, &mut db);
  let before = db.messages(task.id()).expect("messages").len();
  let answer = json!({ "project": "Kontur", "run": run.id().to_string(), "text": "Keep the HubSpot owner." });
  let told = chat_call(&mut served, &scope, "send_message", answer.clone()).expect("proposed");
  assert_eq!(told["waits_for_person"], true, "{told}");
  assert_eq!(db.messages(task.id()).expect("messages").len(), before, "nothing sent");
  let proposed = actions(&db, &scope).remove(0);
  assert_eq!(proposed.outcome, ActionOutcome::Waits);
  assert!(matches!(
    proposed.change.map(|change| change.change),
    Some(Change::Message { text, .. }) if text == "Keep the HubSpot owner."
  ));

  allow(&served, OrchestratorConsent::AnswerAgents);
  let refused = chat_call(&mut served, &scope, "send_message", answer.clone()).expect_err("decision consent required");
  assert!(matches!(&refused, ToolError::Refused { code, .. } if code == "automation_not_allowed"));
  served
    .services()
    .workspace
    .set_automation_answers_approvals(true)
    .expect("decision consent");
  let sent = chat_call(&mut served, &scope, "send_message", answer).expect("sent at once");
  assert_eq!(
    sent["action"], "answer",
    "the composer's answer to the open question: {sent}"
  );
  let messages = db.messages(task.id()).expect("messages");
  let last = messages.last().expect("the answer");
  assert_eq!(last.body().as_str(), "Keep the HubSpot owner.");
  assert!(last.by_orchestrator(), "the task's conversation names the Orchestrator");
  assert_eq!(actions(&db, &scope)[2].outcome, ActionOutcome::Done);
}

#[test]
fn chat_message_that_would_decide_a_plan_is_refused() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let (_, run) = task_run(&db, PLANNING);
  let scope = chat(&served, &mut db);
  allow(&served, OrchestratorConsent::AnswerAgents);
  let refused = chat_call(
    &mut served,
    &scope,
    "send_message",
    json!({ "project": "Kontur", "run": run.id().to_string(), "text": "Approve it." }),
  )
  .expect_err("a plan's decision is the person's");
  assert!(
    matches!(&refused, ToolError::Refused { code, .. } if code == "plan_decision_person_only"),
    "{refused:?}"
  );
  assert!(
    matches!(&actions(&db, &scope)[0].outcome, ActionOutcome::Refused { code, .. } if code == "plan_decision_person_only")
  );
}

#[test]
fn chat_answer_is_a_proposal_until_the_person_lets_the_orchestrator_answer() {
  let (mut served, seeded) = Served::seeded("cards");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let (_, run) = task_run(&db, "Pick where the docs button goes");
  let request = db.questions(run.id()).expect("questions")[0].request.id().to_owned();
  let scope = chat(&served, &mut db);
  let answer = json!({ "project": "Kontur", "run": run.id().to_string(), "choices": ["In the top bar"] });
  let unknown = chat_call(
    &mut served,
    &scope,
    "answer_question",
    json!({ "project": "Kontur", "run": run.id().to_string(), "choices": ["Nowhere"] }),
  )
  .expect_err("an answer the question does not take is not proposed");
  assert!(
    matches!(&unknown, ToolError::Refused { code, .. } if code == "unknown_choice"),
    "{unknown:?}"
  );
  let told = chat_call(&mut served, &scope, "answer_question", answer.clone()).expect("proposed");
  assert_eq!(told["waits_for_person"], true, "{told}");
  assert!(
    db.questions(run.id()).expect("questions")[0].waits(),
    "nothing answered"
  );
  let proposed = actions(&db, &scope).remove(1);
  assert_eq!(proposed.outcome, ActionOutcome::Waits);
  assert!(matches!(
    proposed.change.map(|change| change.change),
    Some(Change::Answer { request: named, choices, .. }) if named == request && choices == ["In the top bar"]
  ));

  allow(&served, OrchestratorConsent::AnswerAgents);
  let refused =
    chat_call(&mut served, &scope, "answer_question", answer.clone()).expect_err("global consent is required too");
  assert!(matches!(&refused, ToolError::Refused { code, .. } if code == "automation_not_allowed"));
  served
    .services()
    .workspace
    .set_automation_answers_approvals(true)
    .expect("decision consent");
  // Sent at once, to the runner: this session has none, so it is refused there, never proposed.
  let refused = chat_call(&mut served, &scope, "answer_question", answer).expect_err("no runner answers here");
  assert!(
    matches!(&refused, ToolError::Refused { code, .. } if code == "runner_unavailable"),
    "{refused:?}"
  );
  assert!(matches!(
    &actions(&db, &scope)[3],
    ChatAction { outcome: ActionOutcome::Refused { code, .. }, change: Some(_), .. } if code == "runner_unavailable"
  ));
}

/// Publishes a checked, signed-in Claude Code connection as what the Orchestrator of the project at `db` runs on.
fn orchestrator_ready(db: &mut ProjectDb, program: PathBuf) {
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Claude · Kontur").expect("name"),
    AgentProvider::ClaudeCodeCli,
    program,
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

/// The waits of the project's latest conversation, each with the message of its turn, if it has one.
fn kept_waits(db: &ProjectDb) -> Vec<(String, Option<ChatMessageId>)> {
  let Some(chat) = db.latest_chat().expect("read") else {
    return Vec::new();
  };
  db.chat_entries(chat)
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Wait { wait, turn } => Some((wait.title, turn.map(|turn| turn.message))),
      _ => None,
    })
    .collect()
}

#[test]
fn chat_starts_no_wait_turn_while_no_runner_attaches() {
  let (folder, seeded) = Served::seed("tasks", None);
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  orchestrator_ready(&mut db, seeded.repo.join("claude.exe"));
  // The person turned *Let the Orchestrator answer agents* on in an earlier session: the project's waits are followed
  // from the start of this one.
  AppDb::open(&seeded.app_database)
    .expect("the app's database")
    .set_orchestrator_answers_agents(true)
    .expect("the person's choice is kept");
  let runner = folder.path().join("no-runner").join("orchester-runner");
  let mut served = Served::start(folder, &seeded.app_database, runner);
  served.tasks_feed(seeded.project);
  served.until("the waits to be kept", |_| {
    kept_waits(&db).iter().any(|(title, _)| title == ASKING)
  });
  // The runner of this session never attaches: following again and again marks no turn thinking for it, so the wait
  // keeps its one turn for a runner that does.
  for _ in 0..200 {
    served.pass();
    thread::sleep(Duration::from_millis(5));
  }
  let kept = kept_waits(&db);
  assert!(
    kept.iter().all(|(_, turn)| turn.is_none()),
    "no wait started a turn: {kept:?}"
  );
  let wait = db
    .chat_entries(db.latest_chat().expect("read").expect("a conversation"))
    .expect("entries")
    .into_iter()
    .find_map(|entry| match entry.kind {
      EntryKind::Wait { wait, .. } if wait.title == ASKING => Some(wait),
      _ => None,
    })
    .expect("the question's wait");
  assert!(
    wait.automatic,
    "it still starts its turn once a runner attaches: {wait:?}"
  );
}

#[test]
fn chat_keeps_the_waits_of_a_project_it_opens_with_their_cards() {
  let (mut served, seeded) = Served::seeded("tasks");
  served.tasks_feed(seeded.project);
  let db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let kinds = |db: &ProjectDb| -> Vec<(WaitKind, String)> {
    let Some(chat) = db.latest_chat().expect("read") else {
      return Vec::new();
    };
    db.chat_entries(chat)
      .expect("entries")
      .into_iter()
      .filter_map(|entry| match entry.kind {
        EntryKind::Wait { wait, .. } => Some((wait.kind, wait.title)),
        _ => None,
      })
      .collect()
  };
  served.until("the waits to be kept", |_| {
    let kept = kinds(&db);
    kept.contains(&(WaitKind::Message, ASKING.to_owned())) && kept.contains(&(WaitKind::Plan, PLANNING.to_owned()))
  });
  let kept = kinds(&db).len();
  for _ in 0..20 {
    served.pass();
  }
  assert_eq!(kinds(&db).len(), kept, "each wait is kept once");
  let chat = db.latest_chat().expect("read").expect("a conversation");
  let seq = db
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .find_map(|entry| match entry.kind {
      EntryKind::Wait { wait, .. } if wait.title == ASKING => Some(entry.seq),
      _ => None,
    })
    .expect("the question's wait");
  let navigation = served.link.navigation().expect("the window attached its navigation");
  navigation.go(orchester_app::routes::Route::Project(
    seeded.project,
    orchester_app::routes::ProjectPage::Orchestrator(orchester_app::routes::OrchestratorChat::Chat(chat)),
  ));
  let card = format!("orchestrator-wait-wait-{seq}");
  served.until("the wait's card", |served| {
    served.tree.get_element_by_id(&card).is_some()
  });
  assert!(
    served.tree.get_element_by_id(&format!("{card}-open")).is_some(),
    "the card opens the task's conversation"
  );
}
