//! What an agent CLI's call of one of Orchester's file tools asked, read from the call's input for its transcript
//! row: Claude Code tells it right after the call's start, Codex before the end of a call that succeeded. An edit's
//! and a write's texts stay whole until the event is redacted, then become diff lines cut to a size; a call whose
//! input the tool refuses, or a tool of another server, tells nothing. The lines follow the shapes in
//! `tests/first_edits.rs`.

use orchester_core::{
  connection::{
    app_server::{AppServerStream, Update},
    stream::{
      AgentEvent, Breaks, CallChange, ClaudeStream, CommandStatus, FinalBreak, MAX_CHANGE_LINES, NativeCall,
      OutputStream,
    },
  },
  diff::{LineKind, MAX_LINE_CHARS},
  native::{edit::LineChanges, tools::ToolName},
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

/// Claude Code's events for `lines`, redacted with `redact` and settled, as the runner records them.
fn claude_redacted(lines: &[String], redact: impl Fn(&str) -> String) -> Vec<AgentEvent> {
  let mut stream = ClaudeStream::default();
  lines
    .iter()
    .flat_map(|line| stream.parse(OutputStream::Stdout, line))
    .filter(|event| !matches!(event, AgentEvent::TurnStarted | AgentEvent::TurnCompleted { .. }))
    .map(|mut event| {
      event.redact(&redact);
      event.settle();
      event
    })
    .collect()
}

fn claude(lines: &[String]) -> Vec<AgentEvent> {
  claude_redacted(lines, str::to_owned)
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

/// The one call `events` tell of: its tool, target and change.
fn only_call(events: &[AgentEvent]) -> (ToolName, &str, Option<&CallChange>) {
  let [(_, call)] = told(events)[..] else {
    panic!("one call told: {events:#?}")
  };
  let input = call.input.as_ref().expect("what a CLI's call asked");
  (call.tool, input.target(), input.change().map(|change| &**change))
}

/// Each hunk's lines, by kind and text.
fn lines(change: &CallChange) -> Vec<Vec<(LineKind, &str)>> {
  change
    .hunks()
    .iter()
    .map(|hunk| hunk.lines().iter().map(|line| (line.kind(), line.text())).collect())
    .collect()
}

/// An `edit_file` call of `path` that replaces its first lines, `expected`, with `content`.
fn edit(path: &str, expected: &str, content: &str) -> String {
  let lines = expected.lines().count();
  tool_use(
    "msg_1",
    "toolu_edit",
    "mcp__Orchester__edit_file",
    &json!({"filename": path, "start_line": 1, "end_line": lines, "expected": expected, "content": content}),
  )
}

fn write(content: &str) -> String {
  tool_use(
    "msg_1",
    "toolu_write",
    "mcp__Orchester__create_file",
    &json!({"filename": "notes/new.md", "content": content}),
  )
}

#[test]
fn cli_file_calls_claude_tells_an_edit_s_diff_right_after_its_start() {
  let events = claude(&[
    edit("src/lib.rs", "fn a() {\n  1\n}\n", "fn a() {\n  2\n  3\n}\n"),
    tool_result(
      "toolu_edit",
      "edited src/lib.rs: lines 1–4 now hold the 4 new lines",
      false,
    ),
  ]);
  assert!(matches!(&events[0], AgentEvent::CommandStarted { item, .. } if item == "toolu_edit"));
  assert!(matches!(&events[1], AgentEvent::NativeCall { item, .. } if item == "toolu_edit"));
  assert!(matches!(
    &events[2],
    AgentEvent::CommandFinished {
      status: CommandStatus::Completed,
      ..
    }
  ));
  let (tool, target, change) = only_call(&events);
  assert_eq!((tool, target), (ToolName::EditFile, "src/lib.rs"));
  let change = change.expect("an edit's change");
  use LineKind::{Added, Context, Removed};
  assert_eq!(
    lines(change),
    [vec![
      (Context, "fn a() {"),
      (Removed, "  1"),
      (Added, "  2"),
      (Added, "  3"),
      (Context, "}")
    ]]
  );
  assert_eq!(change.counts(), Some(LineChanges { added: 2, removed: 1 }));
}

#[test]
fn cli_file_calls_an_edit_shows_shared_lines_as_context_and_a_line_ending_change() {
  use LineKind::{Added, Context};
  // The last shared line only gains its line break.
  let events = claude(&[edit("a.txt", "a\nb", "a\nb\nc")]);
  let change = only_call(&events).2.expect("change");
  assert_eq!(lines(change), [vec![(Context, "a"), (Context, "b"), (Added, "c")]]);
  assert_eq!(change.counts(), Some(LineChanges { added: 1, removed: 0 }));
  assert_eq!(change.hunks()[0].breaks(), Breaks::default());

  let events = claude(&[edit("a.txt", "a\r\nb\r\n", "a\nb\n")]);
  let change = only_call(&events).2.expect("change");
  assert_eq!(lines(change), [vec![(Context, "a"), (Context, "b")]]);
  assert!(
    change.hunks()[0].breaks().line_endings,
    "CRLF to LF is said, not shown as the same lines"
  );
  assert_eq!(
    change.counts(),
    Some(LineChanges { added: 2, removed: 2 }),
    "each line changes, as git counts it"
  );
}

#[test]
fn cli_file_calls_an_edit_says_it_adds_or_removes_the_final_line_break() {
  for (old, new, final_break) in [("x\n", "x", FinalBreak::Removed), ("a\nx", "a\nx\n", FinalBreak::Added)] {
    let events = claude(&[edit("a.txt", old, new)]);
    let change = only_call(&events).2.expect("change");
    assert!(
      lines(change)[0].iter().all(|(kind, _)| *kind == LineKind::Context),
      "{old:?}"
    );
    assert_eq!(change.hunks()[0].breaks().final_break, Some(final_break), "{old:?}");
    assert_eq!(
      change.counts(),
      Some(LineChanges { added: 1, removed: 1 }),
      "{old:?}: not +0 −0"
    );
  }
}

#[test]
fn cli_file_calls_claude_tells_a_write_s_content_and_what_a_read_or_search_is_about() {
  let events = claude(&[
    write("# Notes\r\n\r\nfirst\n"),
    tool_use(
      "msg_2",
      "toolu_read",
      "mcp__Orchester__read_file",
      &json!({"filename": "README.md", "start_line": null, "length": null}),
    ),
    tool_use(
      "msg_3",
      "toolu_search",
      "mcp__Orchester__search_file",
      &json!({"term": "useKeymap", "limit_files": 5}),
    ),
    tool_use(
      "msg_4",
      "toolu_list",
      "mcp__Orchester__list_files",
      &json!({"pattern": null, "path": null}),
    ),
  ]);
  let calls: Vec<(&str, ToolName, &str, bool)> = told(&events)
    .into_iter()
    .map(|(item, call)| {
      let input = call.input.as_ref().expect("input");
      (item, call.tool, input.target(), input.change().is_some())
    })
    .collect();
  assert_eq!(
    calls,
    [
      ("toolu_write", ToolName::WriteFile, "notes/new.md", true),
      ("toolu_read", ToolName::ReadFile, "README.md", false),
      ("toolu_search", ToolName::Search, "useKeymap", false),
      ("toolu_list", ToolName::ListFiles, ".", false),
    ]
  );
  let scopes: Vec<Option<&str>> = told(&events)
    .into_iter()
    .map(|(_, call)| call.input.as_ref().and_then(|input| input.scope()))
    .collect();
  assert_eq!(
    scopes,
    [None, None, None, None],
    "a search of the worktree names no folder"
  );
  let written = told(&events)[0]
    .1
    .input
    .as_ref()
    .and_then(|input| input.change())
    .expect("write");
  assert_eq!(
    lines(written),
    [vec![
      (LineKind::Added, "# Notes"),
      (LineKind::Added, ""),
      (LineKind::Added, "first")
    ]]
  );
  assert_eq!(written.counts(), Some(LineChanges { added: 3, removed: 0 }));

  let empty = claude(&[write("")]);
  assert!(only_call(&empty).2.expect("an empty write").is_empty());
}

#[test]
fn cli_file_calls_tell_nothing_for_refused_input_or_another_server_s_tool() {
  let events = claude(&[
    // An edit that changes nothing is refused: the call fails with that error, which its row shows.
    edit("a.rs", "a", "a"),
    tool_result("toolu_edit", "invalid input for `edit_file`", true),
    tool_use(
      "msg_2",
      "toolu_other",
      "mcp__files__edit_file",
      &json!({"filename": "a.rs", "start_line": 1, "end_line": 1, "expected": "a", "content": "b"}),
    ),
    tool_use("msg_3", "toolu_git", "mcp__Orchester__git_status", &json!({})),
    tool_use(
      "msg_4",
      "toolu_old",
      "mcp__Orchester__write_file",
      &json!({"path": "a.rs", "content": "b"}),
    ),
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
  let events = claude(&[write(&content)]);
  let change = only_call(&events).2.expect("a write's change");
  assert_eq!(
    change.hunks().iter().map(|hunk| hunk.lines().len()).sum::<usize>(),
    MAX_CHANGE_LINES
  );
  assert_eq!(change.omitted(), 50);
  assert_eq!(
    change.counts().map(|counts| counts.added),
    Some(u64::try_from(MAX_CHANGE_LINES + 50).expect("small"))
  );
  let first = &change.hunks()[0].lines()[0];
  assert!(first.cut());
  assert_eq!(first.text().chars().count(), MAX_LINE_CHARS);
}

#[test]
fn cli_file_calls_find_a_secret_over_several_lines_or_past_the_cut() {
  let key = "-----BEGIN PRIVATE KEY-----\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASC\nBKcwggSjAgEAAoIBAQC7VJTUt9Us8cKj\n-----END PRIVATE KEY-----";
  let token = "ghp_0123456789abcdefghijklmnopqrstuvwxyz";
  let redact = |text: &str| text.replace(key, "[secret]").replace(token, "[secret]");
  let content = format!("key = \"\"\"\n{key}\n\"\"\"\n");
  let events = claude_redacted(&[write(&content)], redact);
  let change = only_call(&events).2.expect("a write's change");
  assert_eq!(
    lines(change),
    [vec![
      (LineKind::Added, "key = \"\"\""),
      (LineKind::Added, "[secret]"),
      (LineKind::Added, "\"\"\"")
    ]]
  );

  // The token starts 10 characters before the cut: cut first, its start would no longer match.
  let line = format!("{}{token}", "y".repeat(MAX_LINE_CHARS - 10));
  let events = claude_redacted(&[edit("a.txt", "a", &line)], redact);
  let change = only_call(&events).2.expect("an edit's change");
  let added = &change.hunks()[0].lines()[1];
  assert!(added.text().ends_with("[secret]"), "{}", added.text());
  assert!(!added.text().contains("ghp_"));
}

#[test]
fn cli_file_calls_store_no_whole_text_before_they_settle() {
  let mut stream = ClaudeStream::default();
  let events = stream.parse(OutputStream::Stdout, &write("TOKEN=not-redacted-yet\n"));
  let [(_, call)] = told(&events)[..] else {
    panic!("one call told: {events:#?}")
  };
  let stored = serde_json::to_string(call).expect("serializes");
  assert!(!stored.contains("not-redacted-yet"), "{stored}");
  assert!(!format!("{call:?}").contains("not-redacted-yet"));
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
      Update::Activity(mut event) => {
        event.settle();
        Some(event)
      }
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
  let input = json!({"filename": "README.md", "start_line": 1, "end_line": 1, "expected": "hi", "content": "hi there"});
  let mut sent = Vec::new();
  sent.extend(mcp_call("mcp-1", &input, &json!({"status": "completed"})));
  sent.extend(mcp_call("mcp-failed", &input, &json!({"status": "failed"})));
  sent.extend(mcp_call(
    "mcp-error",
    &input,
    &json!({"status": "completed", "result": {"content": [], "isError": true}}),
  ));
  let events = codex(&sent);
  assert_eq!(told(&events).len(), 1, "a failed call shows no change: {events:#?}");
  let (tool, target, change) = only_call(&events);
  assert_eq!((tool, target), (ToolName::EditFile, "README.md"));
  assert_eq!(
    lines(change.expect("change")),
    [vec![(LineKind::Removed, "hi"), (LineKind::Added, "hi there")]]
  );
  assert!(matches!(&events[0], AgentEvent::NativeCall { item, .. } if item == "mcp-1"));
  assert!(matches!(&events[1], AgentEvent::McpToolFinished { item, .. } if item == "mcp-1"));
}

/// A call is recorded under the name of the file tool the CLI called; a record made under the native agent's older
/// names still reads.
#[test]
fn cli_file_calls_are_recorded_under_the_tools_current_names() {
  for (tool, name, old) in [
    (ToolName::Search, "search_file", "search"),
    (ToolName::WriteFile, "create_file", "write_file"),
  ] {
    assert_eq!(serde_json::to_value(tool).expect("serialized"), json!(name));
    for stored in [name, old] {
      assert_eq!(serde_json::from_value::<ToolName>(json!(stored)).expect("read"), tool);
    }
  }
  assert_eq!(
    serde_json::to_value(ToolName::EditFile).expect("serialized"),
    json!("edit_file")
  );
}
