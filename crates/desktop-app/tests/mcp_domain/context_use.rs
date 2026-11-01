//! How full each agent session's context is (ORC-304): `get_run` gives each step's as `context: {used, window,
//! estimated}`, `null` for a session whose CLI reported nothing, and `get_conversation` gives the current step's; the
//! Orchestrator chat's conversation keeps its session's.

use orchester_core::{
  connection::context_use::ContextUse,
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, RunId},
  orchestrator::Chat,
  run::InvocationKind,
  store::{ActivityStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  orchestrator::chat::{ChatView, context_in},
  tasks::context_uses::of_execution_in,
};
use serde_json::{Value, json};

use super::{Served, kontur::Seeded};

/// Records `reading` for the open agent invocation of the run of `title`. Answers the run's id and the task's key.
fn reported(seeded: &Seeded, title: &str, reading: &ContextUse) -> (String, String) {
  let mut db = ProjectDb::open(&seeded.repo).expect("the database opens");
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.details().title.as_str() == title)
    .expect("the seeded task");
  let run = db
    .runs_of_task(task.id())
    .expect("runs")
    .pop()
    .expect("its run")
    .expect("readable");
  let invocation = db
    .invocations(run.id())
    .expect("invocations")
    .into_iter()
    .find(|invocation| invocation.ended_at.is_none() && matches!(invocation.kind, InvocationKind::Agent { .. }))
    .expect("the agent at work");
  db.save_invocation_context(invocation.id, reading, Timestamp::now())
    .expect("the reading is kept");
  (run.id().to_string(), task.key().to_string())
}

#[test]
fn domain_context_use_of_each_step_and_of_the_conversation() {
  let (mut served, seeded) = Served::seeded("usage");
  let reading = ContextUse::of_model(150_000, "claude-sonnet-4-5");
  let (run, task) = reported(&seeded, "Add retry to webhook sender", &reading);
  let shown = served.ok("get_run", &json!({ "project": "Kontur", "run": &run }));
  let steps = shown["steps"].as_array().expect("steps");
  let expected = json!({ "used": 150_000, "window": 200_000, "estimated": true });
  assert_eq!(
    steps.iter().filter(|step| step["context"] == expected).count(),
    1,
    "the working step's session: {shown}"
  );
  assert!(
    steps
      .iter()
      .all(|step| step["context"] == expected || step["context"] == Value::Null),
    "a session that reported nothing is unknown, not 0%: {shown}"
  );
  let conversation = served.ok("get_conversation", &json!({ "project": "Kontur", "task": &task }));
  assert_eq!(conversation["context"], expected, "{conversation}");
  assert!(conversation["context_step"].is_string(), "{conversation}");
}

#[test]
fn domain_context_use_of_the_orchestrator_conversation() {
  let (_served, seeded) = Served::seeded("usage");
  let mut db = ProjectDb::open(&seeded.repo).expect("the database opens");
  let chat = ChatId::generate();
  let text = MessageBody::new("What runs?").expect("text");
  db.keep_chat_message(
    &Chat::start(chat, &text, Timestamp::now()),
    (ChatMessageId::generate(), &text),
    Timestamp::now(),
  )
  .expect("kept");
  let view = ChatView::load_in(&seeded.repo, chat).expect("read").expect("kept");
  assert_eq!(view.context, None, "unknown until a turn reports it");
  assert_eq!(context_in(&seeded.repo, chat).expect("read"), None);
  let reading = ContextUse::reported(42_000, 258_400);
  db.set_chat_context(chat, &reading).expect("kept");
  let view = ChatView::load_in(&seeded.repo, chat).expect("read").expect("kept");
  assert_eq!(view.context, Some(reading));
  // The meter reads the reading alone as each turn arrives.
  let later = ContextUse::reported(220_000, 258_400);
  db.set_chat_context(chat, &later).expect("kept");
  assert_eq!(context_in(&seeded.repo, chat).expect("read"), Some(later));
}

#[test]
fn domain_context_use_of_the_step_at_work_read_alone_per_turn() {
  let (_served, seeded) = Served::seeded("usage");
  let first = ContextUse::reported(60_000, 200_000);
  let (run, _) = reported(&seeded, "Add retry to webhook sender", &first);
  let db = ProjectDb::open(&seeded.repo).expect("the database opens");
  let run: RunId = run.parse().expect("a run id");
  let invocation = db
    .invocations(run)
    .expect("invocations")
    .into_iter()
    .find(|invocation| invocation.ended_at.is_none() && matches!(invocation.kind, InvocationKind::Agent { .. }))
    .expect("the agent at work");
  assert_eq!(
    of_execution_in(&seeded.repo, run, invocation.execution).expect("read"),
    Some(first)
  );
  let later = ContextUse::reported(170_000, 200_000);
  reported(&seeded, "Add retry to webhook sender", &later);
  assert_eq!(
    of_execution_in(&seeded.repo, run, invocation.execution).expect("read"),
    Some(later),
    "the turn's new reading"
  );
}
