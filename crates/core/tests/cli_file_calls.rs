//! What an agent CLI's call of one of Orchester's file tools asked, read from the call's input for its transcript
//! row: Claude Code tells it right after the call's start, Codex before the end of a call that succeeded. An edit's
//! and a write's change become diff lines, cut to a size; a call whose input the tool refuses, or a tool of another
//! server, tells nothing. The lines follow the shapes in `tests/first_edits.rs`.

use orchester_core::{
  connection::{
    app_server::{AppServerStream, Update},
    stream::{
      AgentEvent, CallChange, CallInput, ChangeLine, ClaudeStream, CommandStatus, MAX_CHANGE_LINES, NativeCall,
      OutputStream,
    },
  },
  diff::{LineKind, MAX_LINE_CHARS},
  native::tools::ToolName,
};
use serde_json::{Value, json};

fn tool_use(message: &str, id: &str, name: &str, input: &Value) -> String {
  json!({"type": "assistant", "message": {
    "id": message, "role": "assistant", "type": "message",
    "content": [{"type": "tool_use", "id": id, "name": name, "input": input}],
    "usage": {"input_tokens": 100, "output_tokens": 10}
  }})
  .to_string()
}

fn tool_result(id: &str, text: &str, is_error: bool) -> String {
  json!({"type": "user", "message": {"role": "user", "content": [
    {"type": "tool_result", "tool_use_id": id, "content": text, "is_error": is_error}
  ]}})
  .to_string()
}

fn claude(lines: &[String]) -> Vec<AgentEvent> {
  let mut stream = ClaudeStream::default();
  lines
    .iter()
    .flat_map(|line| stream.parse(OutputStream::Stdout, line))
    .filter(|event| !matches!(event, AgentEvent::TurnStarted | AgentEvent::TurnCompleted { .. }))
    .collect()
}

fn told(events: &[AgentEvent]) -> Vec<(&str, &NativeCall)> {
  events
    .iter()
    .filter_map(|event| match event {
      AgentEvent::NativeCall { item, call } => Some((item.as_str(), call)),
      _ => None,
    })
    .collect()
}

fn line(kind: LineKind, text: &str) -> ChangeLine {
  ChangeLine {
    kind,
    text: text.into(),
    cut: false,
  }
}

fn asked(tool: ToolName, target: &str, change: Option<CallChange>) -> NativeCall {
  NativeCall {
    tool,
    detail: None,
    declined: None,
    input: Some(CallInput {
      target: target.into(),
      change,
    }),
  }
}

#[test]
fn cli_file_calls_claude_tells_an_edit_s_diff_right_after_its_start() {
  let input = json!({"path": "src/lib.rs", "edits": [
    {"old": "fn a() {\n  1\n}\n", "new": "fn a() {\n  2\n  3\n}\n", "replace_all": null},
    {"old": "use x;", "new": "use y;", "replace_all": true},
  ]});
  let events = claude(&[
    tool_use("msg_1", "toolu_edit", "mcp__Orchester__edit_file", &input),
    tool_result("toolu_edit", "edited src/lib.rs: 2 edit(s) applied", false),
  ]);
  let change = CallChange {
    hunks: vec![
      vec![
        line(LineKind::Context, "fn a() {"),
        line(LineKind::Removed, "  1"),
        line(LineKind::Added, "  2"),
        line(LineKind::Added, "  3"),
        line(LineKind::Context, "}"),
      ],
      vec![line(LineKind::Removed, "use x;"), line(LineKind::Added, "use y;")],
    ],
    omitted: 0,
    added: 3,
    removed: 2,
  };
  assert!(
    matches!(&events[0], AgentEvent::CommandStarted { item, .. } if item == "toolu_edit"),
    "{events:#?}"
  );
  assert_eq!(
    events[1],
    AgentEvent::NativeCall {
      item: "toolu_edit".into(),
      call: asked(ToolName::EditFile, "src/lib.rs", Some(change)),
    }
  );
  assert!(matches!(
    &events[2],
    AgentEvent::CommandFinished {
      status: CommandStatus::Completed,
      ..
    }
  ));
}

#[test]
fn cli_file_calls_claude_tells_a_write_s_content_and_what_a_read_or_search_is_about() {
  let events = claude(&[
    tool_use(
      "msg_1",
      "toolu_write",
      "mcp__Orchester__write_file",
      &json!({"path": "notes/new.md", "content": "# Notes\r\n\r\nfirst\n"}),
    ),
    tool_use(
      "msg_2",
      "toolu_read",
      "mcp__Orchester__read_file",
      &json!({"path": "README.md", "offset": null, "limit": null}),
    ),
    tool_use(
      "msg_3",
      "toolu_search",
      "mcp__Orchester__search",
      &json!({"pattern": "useKeymap", "path": "src", "glob": null, "case_insensitive": null, "context": null}),
    ),
    tool_use(
      "msg_4",
      "toolu_list",
      "mcp__Orchester__list_files",
      &json!({"pattern": null, "path": null}),
    ),
  ]);
  let written = CallChange {
    hunks: vec![vec![
      line(LineKind::Added, "# Notes"),
      line(LineKind::Added, ""),
      line(LineKind::Added, "first"),
    ]],
    omitted: 0,
    added: 3,
    removed: 0,
  };
  assert_eq!(
    told(&events),
    [
      (
        "toolu_write",
        &asked(ToolName::WriteFile, "notes/new.md", Some(written))
      ),
      ("toolu_read", &asked(ToolName::ReadFile, "README.md", None)),
      ("toolu_search", &asked(ToolName::Search, "useKeymap", None)),
      ("toolu_list", &asked(ToolName::ListFiles, ".", None)),
    ]
  );
}

#[test]
fn cli_file_calls_tell_nothing_for_refused_input_or_another_server_s_tool() {
  let events = claude(&[
    // `edits` must hold an edit: the call fails with that error, which its row shows.
    tool_use(
      "msg_1",
      "toolu_invalid",
      "mcp__Orchester__edit_file",
      &json!({"path": "a.rs", "edits": []}),
    ),
    tool_result("toolu_invalid", "invalid input for `edit_file`", true),
    tool_use(
      "msg_2",
      "toolu_other",
      "mcp__files__edit_file",
      &json!({"path": "a.rs", "edits": [{"old": "a", "new": "b", "replace_all": null}]}),
    ),
    tool_use("msg_3", "toolu_git", "mcp__Orchester__git_status", &json!({})),
  ]);
  assert!(told(&events).is_empty(), "{events:#?}");
}

#[test]
fn cli_file_calls_keep_a_change_s_first_lines_and_count_the_rest() {
  let long = "x".repeat(MAX_LINE_CHARS + 5);
  let content: String = std::iter::once(long.as_str())
    .chain(std::iter::repeat_n("line", MAX_CHANGE_LINES + 49))
    .map(|line| format!("{line}\n"))
    .collect();
  let events = claude(&[tool_use(
    "msg_1",
    "toolu_write",
    "mcp__Orchester__write_file",
    &json!({"path": "big.txt", "content": content}),
  )]);
  let [(_, call)] = told(&events)[..] else {
    panic!("one call told: {events:#?}")
  };
  let change = call
    .input
    .as_ref()
    .and_then(|input| input.change.as_ref())
    .expect("a write's change");
  assert_eq!(change.hunks.iter().map(Vec::len).sum::<usize>(), MAX_CHANGE_LINES);
  assert_eq!(change.omitted, 50);
  assert_eq!(change.added, u64::try_from(MAX_CHANGE_LINES + 50).expect("small"));
  let first = &change.hunks[0][0];
  assert!(first.cut);
  assert_eq!(first.text.chars().count(), MAX_LINE_CHARS);
}

#[test]
fn cli_file_calls_redact_the_target_and_every_line() {
  let mut event = AgentEvent::NativeCall {
    item: "toolu_write".into(),
    call: asked(
      ToolName::WriteFile,
      "secret-dir/a.env",
      Some(CallChange {
        hunks: vec![vec![line(LineKind::Added, "TOKEN=secret-value")]],
        omitted: 0,
        added: 1,
        removed: 0,
      }),
    ),
  };
  event.redact(&|text| text.replace("secret", "[redacted]"));
  let AgentEvent::NativeCall { call, .. } = &event else {
    panic!("still a native call")
  };
  let input = call.input.as_ref().expect("input");
  assert_eq!(input.target, "[redacted]-dir/a.env");
  let change = input.change.as_ref().expect("change");
  assert_eq!(change.hunks[0][0].text, "TOKEN=[redacted]-value");
}

#[test]
fn cli_file_calls_read_a_stored_native_call_without_input() {
  let stored = json!({"kind": "native_call", "item": "call_1", "call": {"tool": "edit_file", "detail": {
    "kind": "changed", "change": "modified", "added": 1, "removed": 0
  }}});
  let event: AgentEvent = serde_json::from_value(stored).expect("an event stored before CLI calls were told");
  assert!(matches!(event, AgentEvent::NativeCall { call, .. } if call.input.is_none()));
}

fn codex(lines: &[Value]) -> Vec<AgentEvent> {
  let mut stream = AppServerStream::default();
  lines
    .iter()
    .flat_map(|line| stream.parse(&line.to_string()).expect("a line Codex sends"))
    .filter_map(|update| match update {
      Update::Activity(event) => Some(event),
      _ => None,
    })
    .collect()
}

fn mcp_call(id: &str, arguments: &Value, completed: &Value) -> [Value; 2] {
  let item =
    json!({"id": id, "type": "mcpToolCall", "server": "Orchester", "tool": "edit_file", "arguments": arguments});
  let mut done = item.clone();
  if let (Some(done), Some(fields)) = (done.as_object_mut(), completed.as_object()) {
    done.extend(fields.clone());
  }
  [
    json!({"method": "item/started", "params": {"threadId": "t", "turnId": "turn", "item": item}}),
    json!({"method": "item/completed", "params": {"threadId": "t", "turnId": "turn", "item": done}}),
  ]
}

#[test]
fn cli_file_calls_codex_tells_a_successful_edit_s_diff_before_its_end() {
  let input = json!({"path": "README.md", "edits": [{"old": "hi", "new": "hi there", "replace_all": null}]});
  let mut lines = Vec::new();
  lines.extend(mcp_call("mcp-1", &input, &json!({"status": "completed"})));
  lines.extend(mcp_call("mcp-failed", &input, &json!({"status": "failed"})));
  lines.extend(mcp_call(
    "mcp-error",
    &input,
    &json!({"status": "completed", "result": {"content": [], "isError": true}}),
  ));
  let events = codex(&lines);
  let change = CallChange {
    hunks: vec![vec![line(LineKind::Removed, "hi"), line(LineKind::Added, "hi there")]],
    omitted: 0,
    added: 1,
    removed: 1,
  };
  assert_eq!(
    told(&events),
    [("mcp-1", &asked(ToolName::EditFile, "README.md", Some(change)))],
    "a failed call shows no change"
  );
  assert!(matches!(&events[1], AgentEvent::McpToolFinished { item, .. } if item == "mcp-1"));
}
