//! The tools besides the files: `delete_file` and `move_file`, `find_references`, `get_task`, `save_memory` and
//! `report_progress`, each read strictly; and what `get_task` answers.

use orchester_core::{
  conversation::{Message, MessageBody, Recipient},
  id::{RunId, StepExecutionId, TaskId},
  name::Name,
  orchester_tools::{FileCall, OrchesterTool, TaskView, ToolCall, ToolCallError},
  project::{ProjectKey, TaskKey},
  run::{Answerer, Plan, PlanDecision, PlanReview, PlanVersion, ReviewNote, StepProgress},
  task::{Task, TaskDetails},
  time::Timestamp,
};
use serde_json::{Value, json};

use crate::{parse, reading};

#[test]
fn orchester_tools_read_the_other_tools_strictly() {
  assert_eq!(
    parse("delete_file", &json!({"filename": "old.md"})),
    Ok(ToolCall::File(Box::new(FileCall::Delete("old.md".to_owned()))))
  );
  assert_eq!(
    parse("move_file", &json!({"from": "a.md", "to": "docs/a.md"})),
    Ok(ToolCall::File(Box::new(FileCall::Move {
      from: "a.md".to_owned(),
      to: "docs/a.md".to_owned()
    })))
  );
  assert_eq!(
    parse("find_references", &json!({"name": "parse_config"})),
    Ok(ToolCall::FindReferences {
      name: "parse_config".to_owned()
    })
  );
  assert_eq!(parse("get_task", &Value::Null), Ok(ToolCall::GetTask));
  assert_eq!(
    parse("report_progress", &json!({"text": "  running the tests  "})),
    Ok(ToolCall::ReportProgress(
      StepProgress::new("running the tests").expect("a line")
    ))
  );
  let saved = json!({"kind": "decision", "text": "Use v2.", "key": "api", "tags": ["api"], "importance": "high",
    "scope": "project"});
  assert_eq!(
    parse("save_memory", &saved).map(|call| call.tool()),
    Ok(OrchesterTool::SaveMemory)
  );
}

#[test]
fn orchester_tools_refuse_bad_input_of_the_other_tools() {
  for (name, arguments) in [
    ("delete_file", json!({"filename": " "})),
    ("move_file", json!({"from": "a.md", "to": "a.md"})),
    ("move_file", json!({"from": "a.md"})),
    ("find_references", json!({"name": "parse config"})),
    ("find_references", json!({"name": "1st"})),
    ("get_task", json!({"run": "x"})),
    ("report_progress", json!({"text": " "})),
    ("report_progress", json!({"text": "x".repeat(201)})),
    ("report_progress", json!({"text": "two\nlines"})),
    ("save_memory", json!({"kind": "gossip", "text": "x"})),
    ("save_memory", json!({"kind": "fact", "text": " "})),
    ("save_memory", json!({"kind": "fact", "text": "x", "scope": "run"})),
    (
      "save_memory",
      json!({"kind": "fact", "text": "x", "importance": "urgent"}),
    ),
    (
      "save_memory",
      json!({"kind": "fact", "text": "x", "tags": ["two words"]}),
    ),
  ] {
    assert!(
      matches!(parse(name, &arguments), Err(ToolCallError::Invalid { .. })),
      "{name} {arguments}"
    );
  }
  for (name, arguments) in [
    ("delete_file", json!({"filename": "old.md"})),
    ("move_file", json!({"from": "a.md", "to": "b.md"})),
  ] {
    assert!(
      matches!(
        ToolCall::parse(name, &arguments, &reading()),
        Err(ToolCallError::NotOffered(_))
      ),
      "{name}"
    );
  }
  for name in ["find_references", "get_task", "save_memory", "report_progress"] {
    let tool = OrchesterTool::from_name(name).expect("a tool");
    assert!(reading().policy(tool).is_some(), "{name} runs in a read-only step");
  }
}

fn task() -> Task {
  let details = TaskDetails {
    title: Name::new("Add sign-in").expect("a title"),
    brief: "Users sign in with GitHub.".to_owned(),
    acceptance_criteria: vec!["The button shows".to_owned(), "Tokens are kept".to_owned()],
    pipeline: None,
    limits: None,
  };
  let key = TaskKey::new(ProjectKey::new("KON").expect("a key"), 12.try_into().expect("non-zero"));
  Task::draft(TaskId::generate(), key, details, Timestamp::from_unix_millis(1))
}

fn plan(run: RunId, version: PlanVersion, text: &str, decision: PlanDecision) -> Plan {
  Plan {
    run,
    execution: StepExecutionId::generate(),
    step: orchester_core::resource::StepKey::new("plan").expect("a key"),
    version,
    text: text.to_owned(),
    created_at: Timestamp::from_unix_millis(2),
    review: Some(PlanReview {
      decision,
      by: Answerer::Person,
      at: Timestamp::from_unix_millis(3),
    }),
  }
}

/// `get_task` answers the task, the approved plans and the person's notes to this run, not another run's.
#[test]
fn orchester_tools_get_task_tells_the_task_plans_and_notes() {
  let (run, task) = (RunId::generate(), task());
  let changes = PlanDecision::ChangesRequested {
    note: ReviewNote::new("Keep the old button").expect("a note"),
  };
  let plans = [
    plan(run, PlanVersion::FIRST, "Draft one.", changes),
    plan(run, PlanVersion::FIRST.next(), "Draft two.", PlanDecision::Approved),
  ];
  let body = |text: &str| MessageBody::new(text).expect("a message");
  let messages = [
    Message::from_person(
      task.id(),
      Some(run),
      Recipient::Orchestrator,
      body("Use the blue style."),
      Timestamp::from_unix_millis(4),
    ),
    Message::from_person(
      task.id(),
      Some(RunId::generate()),
      Recipient::Orchestrator,
      body("Another run's note."),
      Timestamp::from_unix_millis(5),
    ),
  ];
  let text = TaskView {
    run,
    task: &task,
    plans: &plans,
    questions: &[],
    messages: &messages,
  }
  .text();
  for told in [
    "# KON-12 · Add sign-in",
    "Users sign in with GitHub.",
    "- The button shows\n- Tokens are kept",
    "## Approved plans\n\n### plan · version 2\n\nDraft two.",
    "changes requested: Keep the old button",
    "- Wrote: Use the blue style.",
  ] {
    assert!(text.contains(told), "{told}:\n{text}");
  }
  assert!(!text.contains("Draft one."), "{text}");
  assert!(!text.contains("Another run's note."), "{text}");
}
