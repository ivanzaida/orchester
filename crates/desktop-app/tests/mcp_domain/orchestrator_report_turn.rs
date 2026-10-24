//! The turn a day's report starts by itself changes nothing (ORC-47), on the seeded Kontur: with *Let the
//! Orchestrator answer agents* and the decision consent on, which send a message at once in any other turn, the
//! chat's `send_message` is kept as a proposal, and a change no card carries (a hold of new starts, a task's edit) is
//! refused, with nothing held or changed.

use orchester_core::{
  id::{ChatId, ChatMessageId},
  orchestrator::{ActionOutcome, Chat, DailyReport, EntryKind},
  store::{ConversationStore, TaskStore},
  time::Timestamp,
  usage::local_date,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  app_mcp::ToolError,
  orchestrator::{report::build_in, tools::ChatScope},
  workspace::OrchestratorConsent,
};
use serde_json::json;

use super::{
  Served,
  orchestrator_chat::{ASKING, actions, allow, chat, chat_call, task_run},
};

/// Today's report of the project at `db`, kept to start a turn in a conversation of its own, whose turn thinks now:
/// the conversation.
pub(super) fn report_turn(db: &mut ProjectDb, scope: &ChatScope) -> ChatId {
  let now = Timestamp::now();
  let built = build_in(&scope.root, Vec::new(), (local_date(now), now)).expect("built");
  let report = DailyReport {
    automatic: true,
    ..built
  };
  let chat = Chat::named(ChatId::generate(), "Daily report", now);
  assert!(db.keep_daily_report(&chat, &report, now).expect("kept"));
  let seq = db
    .chat_entries(chat.id())
    .expect("entries")
    .into_iter()
    .find_map(|entry| matches!(entry.kind, EntryKind::Report { .. }).then_some(entry.seq))
    .expect("the report's entry");
  assert!(
    db.start_wait_turn((chat.id(), seq), ChatMessageId::generate())
      .expect("started"),
    "the report's turn thinks"
  );
  chat.id()
}

fn refusal(error: &ToolError) -> &str {
  match error {
    ToolError::Refused { code, .. } => code,
    other => panic!("a refusal: {other:?}"),
  }
}

#[test]
fn chat_report_turn_only_proposes_even_with_answer_agents_on() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let (task, run) = task_run(&db, ASKING);
  allow(&served, OrchestratorConsent::AnswerAgents);
  served
    .services()
    .workspace
    .set_automation_answers_approvals(true)
    .expect("decision consent");
  let own = chat(&served, &mut db);
  let scope = ChatScope {
    chat: report_turn(&mut db, &own),
    on_its_own: true,
    ..own
  };
  let before = db.messages(task.id()).expect("messages").len();
  let message = json!({ "project": "Kontur", "run": run.id().to_string(), "text": "Keep the HubSpot owner." });
  let told = chat_call(&mut served, &scope, "send_message", message).expect("proposed");
  assert_eq!(told["waits_for_person"], true, "{told}");
  assert_eq!(db.messages(task.id()).expect("messages").len(), before, "nothing sent");
  assert_eq!(actions(&db, &scope).pop().expect("kept").outcome, ActionOutcome::Waits);

  let held = chat_call(&mut served, &scope, "hold_queue", json!({ "project": "Kontur" })).expect_err("refused");
  assert_eq!(refusal(&held), "report_turn");
  assert!(!db.queue_holds().expect("holds").project, "nothing held");
  let title = task.details().title.as_str().to_owned();
  let update = json!({ "project": "Kontur", "key": task.key().to_string(), "title": "Renamed by a report" });
  let updated = chat_call(&mut served, &scope, "update_task", update).expect_err("refused");
  assert_eq!(refusal(&updated), "report_turn");
  let now = db.task(task.id()).expect("read").expect("the task");
  assert_eq!(now.details().title.as_str(), title, "the task is as it was");
  assert!(
    matches!(&actions(&db, &scope).pop().expect("kept").outcome, ActionOutcome::Refused { code, .. } if code == "report_turn"),
    "each refusal is kept"
  );
}
