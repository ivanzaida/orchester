//! The agent's todo list as its CLI reports it: Claude Code's `TodoWrite` calls and Codex App Server's plan updates
//! (`turn/plan/updated`). The `synthetic-*` fixtures are written by hand after the shapes the CLIs print (Claude Code's
//! `TodoWrite` input `todos[]` of `content`, `status` and `activeForm`; App Server's `plan[]` of `step` and
//! `status`); they are not captured output.

use orchester_core::connection::{
  app_server::{AppServerStream, Update},
  stream::{AgentEvent, ClaudeStream, OutputStream, TodoItem, TodoList, TodoState},
};

fn item(text: &str, state: TodoState) -> TodoItem {
  TodoItem {
    text: text.to_owned(),
    state,
  }
}

/// The list each update reports, in order.
fn updates(events: impl IntoIterator<Item = AgentEvent>) -> Vec<Vec<TodoItem>> {
  events
    .into_iter()
    .filter_map(|event| match event {
      AgentEvent::TodoList { items } => Some(items),
      _ => None,
    })
    .collect()
}

fn claude(fixture: &str) -> Vec<AgentEvent> {
  let mut stream = ClaudeStream::default();
  fixture
    .lines()
    .flat_map(|line| stream.parse(OutputStream::Stdout, line))
    .collect()
}

fn codex(fixture: &str) -> Vec<AgentEvent> {
  let mut stream = AppServerStream::default();
  fixture
    .lines()
    .flat_map(|line| stream.parse(line).expect("App Server line"))
    .filter_map(|update| match update {
      Update::Activity(event) => Some(event),
      _ => None,
    })
    .collect()
}

#[test]
fn todo_lists_read_each_claude_todo_write_of_the_agent_but_not_a_subagents() {
  let lists = updates(claude(include_str!("fixtures/claude/synthetic-todo-write.jsonl")));
  assert_eq!(
    lists,
    [
      vec![
        item("Read the stream parser", TodoState::InProgress),
        item("Add the todo event", TodoState::Pending),
        item("Run the tests", TodoState::Pending),
      ],
      vec![
        item("Read the stream parser", TodoState::Done),
        item("Add the todo event", TodoState::InProgress),
        item("Run the tests", TodoState::Pending),
      ],
    ]
  );
}

#[test]
fn todo_lists_keep_claude_todo_write_as_a_command_too() {
  let events = claude(include_str!("fixtures/claude/synthetic-todo-write.jsonl"));
  let started = events
    .iter()
    .filter(|event| matches!(event, AgentEvent::CommandStarted { command, .. } if command == "TodoWrite"))
    .count();
  assert_eq!(started, 3);
}

#[test]
fn todo_lists_read_each_codex_plan_update_with_its_states() {
  let lists = updates(codex(include_str!("fixtures/codex/synthetic-plan-updates.jsonl")));
  assert_eq!(
    lists,
    [
      vec![
        item("Read the stream parser", TodoState::InProgress),
        item("Add the todo event", TodoState::Pending),
        item("Run the tests", TodoState::Pending),
      ],
      vec![
        item("Read the stream parser", TodoState::Done),
        item("Add the todo event", TodoState::Done),
        item("Run the tests", TodoState::InProgress),
      ],
    ]
  );
}

#[test]
fn todo_lists_leave_out_an_update_with_an_item_they_cannot_read() {
  let unknown = r#"{"method":"turn/plan/updated","params":{"threadId":"t","turnId":"u","plan":[{"step":"Read","status":"blocked"}]}}"#;
  let missing = r#"{"type":"assistant","message":{"id":"m","content":[{"type":"tool_use","id":"t","name":"TodoWrite","input":{"todos":[{"status":"pending"}]}}]}}"#;
  assert!(updates(codex(unknown)).is_empty());
  assert!(updates(claude(missing)).is_empty());
}

#[test]
fn todo_lists_read_an_emptied_list_as_no_list() {
  let emptied = r#"{"type":"assistant","message":{"id":"m","content":[{"type":"tool_use","id":"t","name":"TodoWrite","input":{"todos":[]}}]}}"#;
  let lists = updates(claude(emptied));
  assert_eq!(lists, [Vec::<TodoItem>::new()]);
  assert_eq!(TodoList::new(lists[0].clone()), None);
  let kept = TodoList::new(vec![item("Run the tests", TodoState::Done)]).expect("a list");
  assert_eq!(
    serde_json::to_value(&kept).expect("json"),
    serde_json::json!([{"text": "Run the tests", "state": "done"}])
  );
  assert!(serde_json::from_value::<TodoList>(serde_json::json!([])).is_err());
}

#[test]
fn todo_lists_redact_the_items_text() {
  let mut event = AgentEvent::TodoList {
    items: vec![item("Use the token abc123", TodoState::Pending)],
  };
  event.redact(&|text| text.replace("abc123", "[redacted]"));
  assert_eq!(
    updates([event]),
    [vec![item("Use the token [redacted]", TodoState::Pending)]]
  );
}
