//! First edits made through Orchester's own file tools (ORC-69): Claude Code calls them as
//! `mcp__Orchester__edit_file` and `mcp__Orchester__create_file`, Codex as the tools `edit_file` and
//! `create_file` of the MCP server `Orchester`. A successful call reports the file it changed after its
//! own end, so first-edit tracking sees it; a failed one, or a tool of another server, reports none.
//! A change outside the repository is no first edit. The same tracking counts the agent's searches of the
//! code index (ORC-90). The Claude Code lines follow the shapes in
//! `fixtures/claude/synthetic-tool-use.jsonl`; the Codex lines those of `tests/app_server.rs`.

use std::fs;

use orchester_core::{
  connection::{
    app_server::{AppServerStream, Update},
    stream::{AgentEvent, ClaudeStream, CommandStatus, FileChange, FileChangeKind, OutputStream},
  },
  permission::category::CommandScope,
  run::{FirstEdit, FirstEditTracker, TokenUsage},
  time::Timestamp,
};
use serde_json::{Value, json};

const SESSION: &str = "5f0c2a1e-7b3d-4c8e-9a61-2d4b8e0f1a37";

fn tool_use(message: &str, id: &str, name: &str, input: &Value) -> String {
  json!({"type": "assistant", "session_id": SESSION, "message": {
    "id": message, "role": "assistant", "type": "message",
    "content": [{"type": "tool_use", "id": id, "name": name, "input": input}],
    "usage": {"input_tokens": 100, "output_tokens": 10}
  }})
  .to_string()
}

fn tool_result(id: &str, text: &str, is_error: bool) -> String {
  json!({"type": "user", "session_id": SESSION, "message": {"role": "user", "content": [
    {"type": "tool_result", "tool_use_id": id, "content": text, "is_error": is_error}
  ]}})
  .to_string()
}

fn claude(lines: &[String]) -> Vec<AgentEvent> {
  let mut stream = ClaudeStream::default();
  lines
    .iter()
    .flat_map(|line| stream.parse(OutputStream::Stdout, line))
    .collect()
}

fn changed(item: &str, path: &str) -> AgentEvent {
  changed_as(item, path, FileChangeKind::Modified)
}

/// What `create_file` reports: a file added at the path it names.
fn created(item: &str, path: &str) -> AgentEvent {
  changed_as(item, path, FileChangeKind::Added)
}

fn changed_as(item: &str, path: &str, kind: FileChangeKind) -> AgentEvent {
  AgentEvent::FileChanged {
    item: item.into(),
    changes: vec![FileChange {
      path: path.into(),
      kind,
    }],
  }
}

fn finished(item: &str, command: &str, output: &str, status: CommandStatus) -> AgentEvent {
  AgentEvent::CommandFinished {
    item: item.into(),
    command: command.into(),
    output: output.into(),
    exit_code: None,
    status,
  }
}

fn edit_input(path: &str) -> Value {
  json!({"filename": path, "start_line": 1, "end_line": 1, "expected": "hi", "content": "hi there"})
}

#[test]
fn claude_stream_reports_the_change_of_a_successful_orchester_edit_after_its_end() {
  let events = claude(&[
    tool_use(
      "msg_1",
      "toolu_edit",
      "mcp__Orchester__edit_file",
      &edit_input("README.md"),
    ),
    tool_result("toolu_edit", "edited README.md: 1 edit(s) applied", false),
    tool_use(
      "msg_2",
      "toolu_write",
      "mcp__Orchester__create_file",
      &json!({"filename": "notes/new.md", "content": "new notes\n"}),
    ),
    tool_result("toolu_write", "created notes/new.md: 1 lines", false),
  ]);
  let edit = "mcp__Orchester__edit_file README.md";
  let write = "mcp__Orchester__create_file notes/new.md";
  let calls: Vec<&AgentEvent> = events
    .iter()
    // What a call asked for its row is told in `tests/cli_file_calls.rs`.
    .filter(|event| {
      !matches!(
        event,
        AgentEvent::TurnStarted | AgentEvent::TurnCompleted { .. } | AgentEvent::NativeCall { .. }
      )
    })
    .collect();
  assert_eq!(
    calls,
    [
      &AgentEvent::CommandStarted {
        item: "toolu_edit".into(),
        command: edit.into(),
      },
      &finished(
        "toolu_edit",
        edit,
        "edited README.md: 1 edit(s) applied",
        CommandStatus::Completed
      ),
      &changed("toolu_edit", "README.md"),
      &AgentEvent::CommandStarted {
        item: "toolu_write".into(),
        command: write.into(),
      },
      &finished(
        "toolu_write",
        write,
        "created notes/new.md: 1 lines",
        CommandStatus::Completed
      ),
      &created("toolu_write", "notes/new.md"),
    ]
  );
}

#[test]
fn claude_stream_reports_no_change_of_a_failed_orchester_edit_or_another_server_tool() {
  let events = claude(&[
    tool_use(
      "msg_1",
      "toolu_unread",
      "mcp__Orchester__edit_file",
      &edit_input("README.md"),
    ),
    tool_result("toolu_unread", "README.md has not been read", true),
    tool_use(
      "msg_2",
      "toolu_other",
      "mcp__files__edit_file",
      &edit_input("README.md"),
    ),
    tool_result("toolu_other", "edited", false),
    tool_use(
      "msg_3",
      "toolu_read",
      "mcp__Orchester__read_file",
      &json!({"filename": "README.md", "start_line": null, "length": null}),
    ),
    tool_result("toolu_read", "hi", false),
  ]);
  assert!(
    !events
      .iter()
      .any(|event| matches!(event, AgentEvent::FileChanged { .. })),
    "{events:#?}"
  );
  assert!(events.contains(&finished(
    "toolu_unread",
    "mcp__Orchester__edit_file README.md",
    "README.md has not been read",
    CommandStatus::Failed
  )));
}

fn follow(events: &[AgentEvent], in_repository: impl Fn(&str) -> bool) -> FirstEditTracker {
  let mut tracker = FirstEditTracker::default();
  for (at, event) in events.iter().enumerate() {
    tracker.observe(
      event,
      Timestamp::from_unix_millis(i64::try_from(at).expect("small")),
      &in_repository,
    );
  }
  tracker
}

#[test]
fn first_edit_of_a_claude_agent_is_its_first_successful_orchester_edit() {
  let events = claude(&[
    tool_use(
      "msg_1",
      "toolu_unread",
      "mcp__Orchester__edit_file",
      &edit_input("README.md"),
    ),
    tool_result("toolu_unread", "README.md has not been read", true),
    tool_use(
      "msg_2",
      "toolu_edit",
      "mcp__Orchester__edit_file",
      &edit_input("src/lib.rs"),
    ),
    tool_result("toolu_edit", "edited src/lib.rs: 1 edit(s) applied", false),
    tool_use("msg_3", "toolu_later", "Edit", &json!({"file_path": "/repo/README.md"})),
    tool_result("toolu_later", "ok", false),
  ]);
  let tracker = follow(&events, |_| true);
  let progress = tracker.progress();
  let edit = progress.first_edit.as_ref().expect("the agent changed a file");
  assert_eq!(edit.path, "src/lib.rs");
  // The two messages before the edit's result, each 100 input and 10 output tokens.
  assert_eq!(
    progress.before,
    TokenUsage {
      input: 200,
      cached_input: 0,
      output: 20,
    }
  );
}

#[test]
fn first_edit_leaves_out_a_change_outside_the_repository() {
  let events = [
    AgentEvent::TurnStarted,
    changed("1", "/notes/outside.md"),
    AgentEvent::FileChanged {
      item: "2".into(),
      changes: vec![
        FileChange {
          path: "/notes/other.md".into(),
          kind: FileChangeKind::Added,
        },
        FileChange {
          path: "src/lib.rs".into(),
          kind: FileChangeKind::Modified,
        },
      ],
    },
  ];
  let tracker = follow(&events, |path| !path.starts_with("/notes/"));
  assert_eq!(
    tracker.progress().first_edit,
    Some(FirstEdit {
      at: Timestamp::from_unix_millis(2),
      path: "src/lib.rs".into(),
    })
  );
}

#[test]
fn command_scope_tells_a_changed_file_in_the_worktree_from_one_outside() {
  let root = tempfile::tempdir().expect("temporary directory");
  let worktree = root.path().join("worktree");
  let notes = root.path().join("notes");
  fs::create_dir_all(worktree.join("src")).expect("worktree");
  fs::create_dir_all(&notes).expect("writable folder");
  fs::write(worktree.join("src").join("lib.rs"), "x").expect("file");
  fs::write(notes.join("todo.md"), "x").expect("file");
  let scope = CommandScope::new(&worktree, std::slice::from_ref(&notes), None, None);
  let absolute = worktree.join("src").join("lib.rs");
  assert!(scope.is_worktree_file("src/lib.rs"));
  assert!(scope.is_worktree_file(absolute.to_str().expect("a Unicode path")));
  assert!(scope.is_worktree_file("src/new.rs"), "a file not written yet");
  assert!(!scope.is_worktree_file("../notes/todo.md"));
  assert!(!scope.is_worktree_file(notes.join("todo.md").to_str().expect("a Unicode path")));
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

fn mcp_call(id: &str, server: &str, tool: &str, arguments: &Value, completed: &Value) -> [Value; 2] {
  let item = json!({"id": id, "type": "mcpToolCall", "server": server, "tool": tool, "arguments": arguments});
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
fn codex_stream_reports_the_change_of_a_successful_orchester_edit_after_its_end() {
  let completed = json!({"status": "completed"});
  let mut lines = Vec::new();
  lines.extend(mcp_call(
    "mcp-1",
    "Orchester",
    "edit_file",
    &edit_input("README.md"),
    &completed,
  ));
  lines.extend(mcp_call(
    "mcp-2",
    "Orchester",
    "create_file",
    &json!({"filename": "new.md", "content": "x"}),
    &completed,
  ));
  let finished = |item: &str, tool: &str| AgentEvent::McpToolFinished {
    item: item.into(),
    server: "Orchester".into(),
    tool: tool.into(),
    status: CommandStatus::Completed,
    asked: false,
  };
  let events: Vec<AgentEvent> = codex(&lines)
    .into_iter()
    .filter(|event| !matches!(event, AgentEvent::NativeCall { .. }))
    .collect();
  assert_eq!(
    events,
    [
      finished("mcp-1", "edit_file"),
      changed("mcp-1", "README.md"),
      finished("mcp-2", "create_file"),
      created("mcp-2", "new.md"),
    ]
  );
}

#[test]
fn codex_stream_reports_no_change_of_a_failed_orchester_edit_or_another_server_tool() {
  let mut lines = Vec::new();
  lines.extend(mcp_call(
    "mcp-failed",
    "Orchester",
    "edit_file",
    &edit_input("README.md"),
    &json!({"status": "failed"}),
  ));
  lines.extend(mcp_call(
    "mcp-error",
    "Orchester",
    "edit_file",
    &edit_input("README.md"),
    &json!({"status": "completed", "result": {"content": [], "isError": true}}),
  ));
  lines.extend(mcp_call(
    "mcp-other",
    "files",
    "write_file",
    &json!({"path": "new.md", "content": "x"}),
    &json!({"status": "completed"}),
  ));
  let events = codex(&lines);
  assert_eq!(events.len(), 3, "{events:#?}");
  assert!(
    events
      .iter()
      .all(|event| matches!(event, AgentEvent::McpToolFinished { .. })),
    "{events:#?}"
  );
}

fn follow_with_index(events: &[AgentEvent], index_given: bool) -> Option<u64> {
  let mut tracker = FirstEditTracker::new(index_given);
  for event in events {
    tracker.observe(event, Timestamp::from_unix_millis(0), |_| true);
  }
  tracker.progress().index_searches
}

fn codex_command(id: &str, command: &str, status: &str) -> [Value; 2] {
  let item = json!({"id": id, "type": "commandExecution", "command": command, "cwd": "/repo"});
  let mut done = item.clone();
  done["status"] = json!(status);
  done["aggregatedOutput"] = json!("");
  [
    json!({"method": "item/started", "params": {"threadId": "t", "turnId": "turn", "item": item}}),
    json!({"method": "item/completed", "params": {"threadId": "t", "turnId": "turn", "item": done}}),
  ]
}

/// ORC-90: a Codex agent's searches of the code index count, whether through Orchester's index tools or `rg` in the
/// index folder as the skill shows; a command elsewhere, a declined one, a failed tool call and a tool that does not
/// read the index do not.
#[test]
fn index_searches_of_a_codex_agent_count_its_index_tools_and_commands_in_the_index() {
  let completed = json!({"status": "completed"});
  let mut lines = Vec::new();
  lines.extend(mcp_call(
    "mcp-1",
    "Orchester",
    "find_symbol",
    &json!({"name": "parse"}),
    &completed,
  ));
  lines.extend(mcp_call(
    "mcp-2",
    "Orchester",
    "search_file",
    &json!({"term": "parse"}),
    &completed,
  ));
  lines.extend(mcp_call(
    "mcp-3",
    "Orchester",
    "read_file",
    &json!({"filename": "a.rs"}),
    &completed,
  ));
  lines.extend(mcp_call(
    "mcp-4",
    "Orchester",
    "find_symbol",
    &json!({"name": "x"}),
    &json!({"status": "failed"}),
  ));
  lines.extend(codex_command(
    "cmd-1",
    r#""C:\Program Files\PowerShell\7\pwsh.exe" -Command "rg -n '^parse\t' 'H:\repo\.orchester\index\4d2b8e0\symbols.tsv'""#,
    "completed",
  ));
  lines.extend(codex_command(
    "cmd-2",
    r#"pwsh -Command "$IDX = 'H:\repo\.Orchester\Index\4d2b8e0'; rg -n 'x' "$IDX/tree.txt"""#,
    "failed",
  ));
  lines.extend(codex_command("cmd-3", "rg -n 'parse' src", "completed"));
  lines.extend(codex_command(
    "cmd-4",
    "rg -n x /repo/.orchester/index/4d2b8e0/docs.md",
    "declined",
  ));
  let events = codex(&lines);
  assert_eq!(follow_with_index(&events, true), Some(4), "{events:#?}");
  assert_eq!(
    follow_with_index(&events, false),
    Some(0),
    "an agent without the index searches none"
  );
}

/// A Claude Code agent's index tools and its reads in the index folder count, its sub-agent's too: Claude Code prints
/// a sub-agent's tool calls in its own stream, marked with the `Task` call they belong to.
#[test]
fn index_searches_of_a_claude_agent_count_its_sub_agents_too() {
  let sub_agent = |line: String| {
    let mut value: Value = serde_json::from_str(&line).expect("a line");
    value["parent_tool_use_id"] = json!("toolu_task");
    value.to_string()
  };
  let events = claude(&[
    tool_use(
      "msg_1",
      "toolu_find",
      "mcp__Orchester__find_symbol",
      &json!({"name": "Store"}),
    ),
    tool_result("toolu_find", "src/store.rs:4 struct Store", false),
    tool_use(
      "msg_2",
      "toolu_task",
      "Task",
      &json!({"description": "Explore", "prompt": "find the store"}),
    ),
    sub_agent(tool_use(
      "msg_3",
      "toolu_read",
      "Read",
      &json!({"file_path": "/repo/.orchester/index/4d2b8e0/outline/src/store.rs.md"}),
    )),
    sub_agent(tool_result("toolu_read", "- L4-40 struct Store", false)),
    sub_agent(tool_use(
      "msg_4",
      "toolu_list",
      "mcp__Orchester__list_files",
      &json!({"path": "src", "pattern": null}),
    )),
    sub_agent(tool_result("toolu_list", "src/store.rs", false)),
    tool_use("msg_5", "toolu_bash", "Bash", &json!({"command": "git status --short"})),
    tool_result("toolu_bash", "", false),
    tool_result("toolu_task", "the store is in src/store.rs", false),
  ]);
  assert_eq!(follow_with_index(&events, true), Some(3), "{events:#?}");
}
