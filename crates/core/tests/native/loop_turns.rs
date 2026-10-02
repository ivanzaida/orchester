//! The native loop through ordinary turns: order and concurrency of tool calls, results, approvals,
//! refusals, and how the loop ends.

use orchester_core::{
  connection::stream::{AgentEvent, CommandStatus},
  native::{
    item::{Item, UserOrigin},
    model::{ModelError, StopReason},
    session::{DENIED, LoopAction, LoopEnd, LoopError, LoopFailure, LoopInput, RECORDED, REMINDER, ToolWork},
  },
  permission::RepositoryAccess,
  run::StepOutcome,
};
use serde_json::json;

use crate::support::{
  Layout, assistant, call, ended, ok, persisted, planned, request, response, run_ids, start, tool_call,
};

fn finish() -> Item {
  tool_call("finish", "finish_step", json!({"outcome": "done", "summary": "Done."}))
}

fn events(actions: &[LoopAction]) -> Vec<AgentEvent> {
  actions
    .iter()
    .filter_map(|action| match action {
      LoopAction::Emit(events) => Some(events.clone()),
      _ => None,
    })
    .flatten()
    .collect()
}

#[test]
fn native_loop_start() {
  let layout = Layout::new();
  let (native, actions) = start(&layout, RepositoryAccess::Write, &[]);
  let task = Item::user("Do the task.", UserOrigin::Task);
  assert_eq!(actions[0], LoopAction::Persist(vec![task.clone()]));
  assert!(matches!(
    &events(&actions)[..],
    [AgentEvent::SessionStarted { .. }, AgentEvent::TurnStarted]
  ));
  let request = request(&actions).expect("first request");
  assert_eq!(request.items, vec![task]);
  assert_eq!(request.system, "You are a native agent.");
  assert!(!native.has_ended());
}

#[test]
fn native_loop_reads_together_then_writes_in_order() {
  let layout = Layout::new();
  let (mut native, _) = start(&layout, RepositoryAccess::Write, &["cargo build"]);
  let items = vec![
    assistant("Looking."),
    tool_call("r1", "read_file", json!({"path": "src/main.rs"})),
    tool_call(
      "e1",
      "edit_file",
      json!({"path": "src/main.rs", "edits": [{"old": "fn", "new": "pub fn"}]}),
    ),
    tool_call("c1", "run_command", json!({"command": "cargo build"})),
    tool_call("s1", "search", json!({"pattern": "main"})),
  ];
  let actions = native
    .handle(LoopInput::Responded(response(items.clone(), StopReason::ToolUse, 100)))
    .expect("response");
  assert_eq!(
    actions[0],
    LoopAction::Persist(items),
    "the response is stored before its tools run"
  );
  assert!(events(&actions).contains(&AgentEvent::Message {
    text: "Looking.".to_owned()
  }));
  assert_eq!(run_ids(&actions), ["r1", "s1"]);

  let actions = native
    .handle(LoopInput::ToolDone(call("s1"), ok("1 match")))
    .expect("s1");
  assert!(run_ids(&actions).is_empty(), "r1 still runs");
  let actions = native
    .handle(LoopInput::ToolDone(call("r1"), ok("1 fn main")))
    .expect("r1");
  assert_eq!(run_ids(&actions), ["e1"]);
  let actions = native
    .handle(LoopInput::ToolDone(call("e1"), ok("edited")))
    .expect("e1");
  let command = planned(&actions);
  let [tool] = &command[..] else { panic!("one command") };
  let ToolWork::Command(parsed) = &tool.work else {
    panic!("command")
  };
  assert_eq!(parsed.text(), "cargo build");

  let actions = native.handle(LoopInput::ToolDone(call("c1"), ok("built"))).expect("c1");
  let request = request(&actions).expect("next request");
  let results: Vec<&str> = request
    .items
    .iter()
    .filter_map(|item| match item {
      Item::ToolResult { id, .. } => Some(id.as_str()),
      _ => None,
    })
    .collect();
  assert_eq!(results, ["s1", "r1", "e1", "c1"]);
}

#[test]
fn native_loop_tool_events() {
  let layout = Layout::new();
  let (mut native, _) = start(&layout, RepositoryAccess::Write, &[]);
  let read = tool_call(
    "r1",
    "read_file",
    json!({"path": "src/main.rs", "offset": 3, "limit": 10}),
  );
  let actions = native
    .handle(LoopInput::Responded(response(vec![read], StopReason::ToolUse, 10)))
    .expect("response");
  assert!(events(&actions).contains(&AgentEvent::CommandStarted {
    item: "r1".to_owned(),
    command: "read_file src/main.rs:3-12".to_owned(),
  }));
  let mut outcome = ok("lines");
  outcome.activity = "10 lines".to_owned();
  let actions = native.handle(LoopInput::ToolDone(call("r1"), outcome)).expect("done");
  assert!(events(&actions).contains(&AgentEvent::CommandFinished {
    item: "r1".to_owned(),
    command: "read_file src/main.rs:3-12".to_owned(),
    output: "10 lines".to_owned(),
    exit_code: None,
    status: CommandStatus::Completed,
  }));
}

#[test]
fn native_loop_finish_ends_and_skips_later_calls() {
  let layout = Layout::new();
  let (mut native, _) = start(&layout, RepositoryAccess::Write, &[]);
  let items = vec![
    tool_call("r1", "read_file", json!({"path": "src/main.rs"})),
    finish(),
    tool_call("r2", "read_file", json!({"path": "src/lib.rs"})),
  ];
  let actions = native
    .handle(LoopInput::Responded(response(items, StopReason::ToolUse, 10)))
    .expect("response");
  assert_eq!(run_ids(&actions), ["r1"], "calls after finish_step do not run");
  let actions = native.handle(LoopInput::ToolDone(call("r1"), ok("text"))).expect("r1");
  let Some(LoopEnd::Result(result)) = ended(&actions) else {
    panic!("result: {actions:?}")
  };
  assert_eq!((result.outcome, result.summary.as_str()), (StepOutcome::Done, "Done."));
  assert_eq!(
    persisted(&actions),
    [
      Item::tool_result(call("r1"), "text", false),
      Item::tool_result(call("finish"), RECORDED, false)
    ],
    "the accepted result is stored as the call's result"
  );
  assert!(native.has_ended());
}

#[test]
fn native_loop_invalid_calls_get_error_results() {
  let layout = Layout::new();
  let (mut native, _) = start(&layout, RepositoryAccess::ReadOnly, &[]);
  let items = vec![
    tool_call("f1", "finish_step", json!({"outcome": "done"})),
    tool_call("w1", "write_file", json!({"path": "a.rs", "content": "x"})),
    tool_call("u1", "browse", json!({})),
  ];
  let actions = native
    .handle(LoopInput::Responded(response(items, StopReason::ToolUse, 10)))
    .expect("response");
  let errors: Vec<(String, String)> = persisted(&actions)
    .into_iter()
    .filter_map(|item| match item {
      Item::ToolResult {
        id,
        content,
        is_error: true,
      } => Some((id.as_str().to_owned(), content)),
      _ => None,
    })
    .collect();
  assert_eq!(errors.len(), 3);
  assert!(errors[0].1.contains("summary"));
  assert!(
    errors[1].1.contains("not available"),
    "read-only steps get no write_file"
  );
  assert!(errors[2].1.contains("browse"));
  assert!(request(&actions).is_some(), "the loop asks the model again");
}

#[test]
fn native_loop_question_ends() {
  let layout = Layout::new();
  let (mut native, _) = start(&layout, RepositoryAccess::Write, &[]);
  let ask = tool_call(
    "q1",
    "ask_question",
    json!({"question": "Which database?", "options": ["sqlite", "pg"]}),
  );
  let actions = native
    .handle(LoopInput::Responded(response(vec![ask], StopReason::ToolUse, 10)))
    .expect("response");
  let Some(LoopEnd::Question { call: asked, question }) = ended(&actions) else {
    panic!("question")
  };
  assert_eq!(asked, &call("q1"));
  assert_eq!((question.question(), question.options().len()), ("Which database?", 2));
}

#[test]
fn native_loop_command_approval() {
  let layout = Layout::new();
  let (mut native, _) = start(&layout, RepositoryAccess::Write, &["cargo test"]);
  let items = vec![
    tool_call("c1", "run_command", json!({"command": "npm install"})),
    tool_call("c2", "run_command", json!({"command": "npm test"})),
  ];
  let actions = native
    .handle(LoopInput::Responded(response(items, StopReason::ToolUse, 10)))
    .expect("response");
  let Some(LoopAction::Ask(ask)) = actions.last() else {
    panic!("ask: {actions:?}")
  };
  assert_eq!((ask.call.as_str(), ask.command.text()), ("c1", "npm install"));

  let actions = native.handle(LoopInput::Decided(call("c1"), true)).expect("approved");
  assert_eq!(run_ids(&actions), ["c1"]);
  let actions = native
    .handle(LoopInput::ToolDone(call("c1"), ok("installed")))
    .expect("c1");
  assert!(matches!(actions.last(), Some(LoopAction::Ask(ask)) if ask.call == call("c2")));

  assert_eq!(
    native.handle(LoopInput::Decided(call("c1"), false)),
    Err(LoopError::UnknownCall(call("c1")))
  );
  let actions = native.handle(LoopInput::Decided(call("c2"), false)).expect("denied");
  assert_eq!(persisted(&actions), vec![Item::tool_result(call("c2"), DENIED, true)]);
  assert!(events(&actions).contains(&AgentEvent::PermissionDenied {
    item: "c2".to_owned(),
    command: "npm test".to_owned(),
  }));
  assert!(request(&actions).is_some(), "a denial does not end the step");
}

#[test]
fn native_loop_refused_command_continues() {
  let layout = Layout::new();
  let (mut native, _) = start(&layout, RepositoryAccess::Write, &["ls"]);
  let items = vec![
    tool_call("c1", "run_command", json!({"command": "ls | wc"})),
    tool_call("c2", "run_command", json!({"command": "ls"})),
  ];
  let actions = native
    .handle(LoopInput::Responded(response(items, StopReason::ToolUse, 10)))
    .expect("response");
  let stored = persisted(&actions);
  assert!(
    matches!(stored.last(), Some(Item::ToolResult { id, is_error: true, content }) if *id == call("c1") && content.contains("shell: true"))
  );
  assert!(
    events(&actions)
      .iter()
      .any(|event| matches!(event, AgentEvent::PermissionDenied { item, .. } if item == "c1"))
  );
  assert_eq!(run_ids(&actions), ["c2"]);
}

#[test]
fn native_loop_reminder_then_no_result() {
  let layout = Layout::new();
  let (mut native, _) = start(&layout, RepositoryAccess::Write, &[]);
  let actions = native
    .handle(LoopInput::Responded(response(
      vec![assistant("All done.")],
      StopReason::EndTurn,
      10,
    )))
    .expect("first miss");
  assert!(persisted(&actions).contains(&Item::user(REMINDER, UserOrigin::Reminder)));
  assert!(request(&actions).is_some());
  let actions = native
    .handle(LoopInput::Responded(response(
      vec![assistant("Really done.")],
      StopReason::EndTurn,
      10,
    )))
    .expect("second miss");
  assert_eq!(ended(&actions), Some(&LoopEnd::NoResult));
}

#[test]
fn native_loop_reminder_after_every_turn_with_calls() {
  let layout = Layout::new();
  let (mut native, _) = start(&layout, RepositoryAccess::Write, &[]);
  let miss = || response(vec![assistant("All done.")], StopReason::EndTurn, 10);
  native.handle(LoopInput::Responded(miss())).expect("first miss");
  let read = tool_call("r1", "read_file", json!({"path": "src/main.rs"}));
  native
    .handle(LoopInput::Responded(response(vec![read], StopReason::ToolUse, 10)))
    .expect("read");
  native.handle(LoopInput::ToolDone(call("r1"), ok("text"))).expect("r1");
  let actions = native.handle(LoopInput::Responded(miss())).expect("later miss");
  assert!(
    persisted(&actions).contains(&Item::user(REMINDER, UserOrigin::Reminder)),
    "a miss after a turn with calls is reminded again: {actions:?}"
  );
  assert_eq!(ended(&actions), None);
}

#[test]
fn native_loop_result_from_final_text() {
  let layout = Layout::new();
  let (mut native, _) = start(&layout, RepositoryAccess::Write, &[]);
  native
    .handle(LoopInput::Responded(response(
      vec![assistant("Done.")],
      StopReason::EndTurn,
      10,
    )))
    .expect("first miss");
  let text = "Here it is:\n{\"outcome\":\"failed\",\"summary\":\"Tests fail.\"}";
  let actions = native
    .handle(LoopInput::Responded(response(
      vec![assistant(text)],
      StopReason::EndTurn,
      10,
    )))
    .expect("second miss");
  let Some(LoopEnd::Result(result)) = ended(&actions) else {
    panic!("result")
  };
  assert_eq!(result.outcome, StepOutcome::Failed);
}

#[test]
fn native_loop_failures() {
  let layout = Layout::new();
  let (mut native, _) = start(&layout, RepositoryAccess::Write, &[]);
  let refusal = StopReason::Refusal {
    category: Some("cyber".to_owned()),
  };
  let actions = native
    .handle(LoopInput::Responded(response(Vec::new(), refusal, 10)))
    .expect("refusal");
  assert_eq!(
    ended(&actions),
    Some(&LoopEnd::Failed(LoopFailure::Refused {
      category: Some("cyber".to_owned())
    }))
  );
  let (mut native, _) = start(&layout, RepositoryAccess::Write, &[]);
  let actions = native
    .handle(LoopInput::ModelFailed(ModelError::KeyRejected))
    .expect("failure");
  assert_eq!(
    ended(&actions),
    Some(&LoopEnd::Failed(LoopFailure::Model(ModelError::KeyRejected)))
  );
  assert!(matches!(
    native.handle(LoopInput::ToolDone(call("x"), ok("x"))),
    Err(LoopError::Unexpected { .. })
  ));
}
