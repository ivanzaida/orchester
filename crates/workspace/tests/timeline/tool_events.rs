//! Tool events in the timeline: which transcript entries are tool events and which kind, the command
//! a CLI wrapped in its shell, what a tool group's summary counts, the change of Orchester's edit tool
//! shown by its call's own row (which carries what the call asked), and a tool step's command, which is a row
//! of its own when it fails and names its header.

use std::sync::Arc;

use orchester_core::{
  connection::stream::{AgentEvent, CommandStatus, ToolCommandResult},
  id::StepExecutionId,
  orchester_tools::OrchesterTool,
  run::TokenUsage,
};
use orchester_desktop::tasks::{
  timeline::{
    ToolRun,
    group::{GroupSummary, ToolKind, shown_as_card, tool_kind, unwrapped},
  },
  transcript::EntryKind,
};
use serde_json::json;

use crate::{at, command, edit, transcript};

#[test]
fn timeline_finds_the_command_a_cli_wrapped_in_its_shell() {
  // Codex 0.159.2 on Windows (probe 2026-10-02).
  let codex = r#""C:\Program Files\PowerShell\7\pwsh.exe" -Command "python -c 1""#;
  assert_eq!(unwrapped(codex), "python -c 1");
  assert_eq!(unwrapped("/bin/zsh -lc 'cargo test -p core'"), "cargo test -p core");
  assert_eq!(unwrapped("bash -c rg cancel"), "rg cancel");
  assert_eq!(unwrapped("pnpm test src/auth"), "pnpm test src/auth");
  assert_eq!(unwrapped("pwsh"), "pwsh");
}

#[test]
fn timeline_tool_events_are_commands_reads_searches_edits_and_notes() {
  let execution = StepExecutionId::generate();
  let mut events = Vec::new();
  for (item, text) in [
    ("1", "Read src/auth.ts"),
    ("2", "Grep cancel"),
    ("3", r#""pwsh.exe" -Command "rg --files src""#),
    ("4", "cat README.md"),
    ("5", "git grep cancel"),
    ("6", "pnpm test"),
  ] {
    events.extend(command(item, text, 0));
  }
  events.push(edit("7", "src/auth.ts"));
  events.push(AgentEvent::Reasoning { text: "Think.".into() });
  events.push(AgentEvent::TurnCompleted {
    usage: TokenUsage::default(),
  });
  events.push(AgentEvent::Message { text: "Done.".into() });
  events.push(AgentEvent::Error {
    message: "Broke.".into(),
  });
  events.push(AgentEvent::PermissionDenied {
    item: "8".into(),
    command: "curl https://example.com".into(),
  });
  let kinds: Vec<Option<ToolKind>> = transcript(execution, events).iter().map(tool_kind).collect();
  assert_eq!(
    kinds,
    [
      Some(ToolKind::Read),
      Some(ToolKind::Search),
      Some(ToolKind::Search),
      Some(ToolKind::Read),
      Some(ToolKind::Search),
      Some(ToolKind::Command),
      Some(ToolKind::Edit),
      Some(ToolKind::Note),
      Some(ToolKind::Note),
      None,
      None,
      // A denied command is a problem row, never grouped.
      None,
    ]
  );
}

#[test]
fn timeline_a_group_counts_by_kind_and_names_a_failed_command() {
  let execution = StepExecutionId::generate();
  let mut events = Vec::new();
  events.extend(command("1", "Read a.rs", 0));
  events.extend(command("2", "Read b.rs", 0));
  events.extend(command("3", "Grep cancel", 0));
  events.extend(command("4", r#"bash -lc "pnpm test src/auth""#, 1));
  events.push(edit("5", "a.rs"));
  events.push(edit("6", "a.rs"));
  events.push(edit("7", "b.rs"));
  events.push(AgentEvent::TurnCompleted {
    usage: TokenUsage::default(),
  });
  let summary = GroupSummary::of(&transcript(execution, events));
  assert_eq!((summary.reads, summary.searches, summary.commands), (2, 1, 1));
  assert_eq!(summary.named, [Arc::<str>::from("pnpm test src/auth")]);
  assert_eq!(summary.failed, Some(("pnpm test src/auth".into(), Some(1))));
  assert_eq!(summary.edits, 2, "files edited, each once");
  assert_eq!(summary.calls, 7, "usage is no tool call");
  assert_eq!(summary.running, None);
  assert_eq!((summary.first, summary.last), (Some(at(1)), Some(at(12))));

  // A group whose command still runs ends with it.
  let growing = transcript(
    execution,
    vec![AgentEvent::CommandStarted {
      item: "9".into(),
      command: "pnpm test".into(),
    }],
  );
  assert_eq!(GroupSummary::of(&growing).running, Some("pnpm test".into()));
}

#[test]
fn timeline_a_tool_step_s_failing_command_is_a_row_of_its_own() {
  let execution = StepExecutionId::generate();
  let ran = |item: &str, result: ToolCommandResult, exit: i64| {
    [
      AgentEvent::ToolCommandStarted {
        tool: "Run checks".into(),
        command: format!("pnpm test {item}"),
      },
      AgentEvent::ToolCommandFinished {
        tool: "Run checks".into(),
        command: format!("pnpm test {item}"),
        result,
        exit_code: Some(exit),
        output: "1 failed".into(),
        omitted_bytes: 0,
      },
    ]
  };
  let mut events = Vec::new();
  events.extend(ran("a", ToolCommandResult::Passed, 0));
  events.extend(ran("b", ToolCommandResult::Failed, 1));
  let kinds: Vec<Option<ToolKind>> = transcript(execution, events).iter().map(tool_kind).collect();
  assert_eq!(
    kinds,
    [Some(ToolKind::Command), None],
    "what failed the step is never grouped"
  );
}

#[test]
fn timeline_a_tool_step_s_header_names_its_last_command_and_exit_code() {
  let (execution, other) = (StepExecutionId::generate(), StepExecutionId::generate());
  let ran = |command: &str, exit: i64| {
    [
      AgentEvent::ToolCommandStarted {
        tool: "Run checks".into(),
        command: command.into(),
      },
      AgentEvent::ToolCommandFinished {
        tool: "Run checks".into(),
        command: command.into(),
        result: ToolCommandResult::Passed,
        exit_code: Some(exit),
        output: "ok".into(),
        omitted_bytes: 0,
      },
    ]
  };
  let mut entries = transcript(execution, [ran("pnpm lint", 0), ran("pnpm test", 3)].concat());
  entries.extend(transcript(other, ran("cargo test", 0).to_vec()));
  assert_eq!(
    ToolRun::of(&entries, execution),
    Some(ToolRun {
      command: "pnpm test".into(),
      exit_code: Some(3),
    })
  );
  assert_eq!(ToolRun::of(&entries, StepExecutionId::generate()), None);
}

/// Claude Code's call of Orchester's `edit_file` is a command row, and Codex's an MCP call row; the change each
/// reports after its end (for first-edit tracking) is not shown again.
#[test]
fn timeline_shows_the_change_of_orchester_s_edit_tool_by_its_call_s_row() {
  let execution = StepExecutionId::generate();
  let mut events = command("toolu_edit", "mcp__Orchester__edit_file README.md", 0).to_vec();
  events.push(edit("toolu_edit", "README.md"));
  events.push(AgentEvent::McpToolFinished {
    item: "mcp-1".into(),
    server: "Orchester".into(),
    tool: "write_file".into(),
    status: CommandStatus::Completed,
    asked: false,
  });
  events.push(edit("mcp-1", "new.md"));
  events.push(edit("toolu_own", "src/lib.rs"));
  let entries = transcript(execution, events);
  let kinds: Vec<&str> = entries
    .iter()
    .map(|entry| match &entry.kind {
      EntryKind::Command(_) => "command",
      EntryKind::McpTool(_) => "mcp",
      EntryKind::FileChange(change) => change.path.as_str(),
      _ => "other",
    })
    .collect();
  assert_eq!(
    kinds,
    ["command", "mcp", "src/lib.rs"],
    "Claude Code's own edit stays a change row"
  );
}

/// What Claude Code's and Codex's calls of Orchester's `edit_file` asked goes with the call's own row, which counts
/// as an edit of its file; the change reported after it stays hidden, and a failed call stays the command it was.
#[test]
fn timeline_a_cli_s_orchester_edit_carries_its_change_and_counts_as_an_edit() {
  let execution = StepExecutionId::generate();
  let input = json!({"path": "README.md", "edits": [{"old": "hi", "new": "hi there", "replace_all": null}]});
  // Settled, as the runner stores it.
  let mut settled = AgentEvent::NativeCall {
    item: String::new(),
    call: OrchesterTool::EditFile.shown_call(&input).expect("an edit's input"),
  };
  settled.settle();
  let AgentEvent::NativeCall { call: asked, .. } = settled else {
    unreachable!("settling keeps the event's kind")
  };
  assert!(asked.input.as_ref().and_then(|input| input.change()).is_some());
  let told = |item: &str| AgentEvent::NativeCall {
    item: item.into(),
    call: asked.clone(),
  };
  let started = |item: &str| AgentEvent::CommandStarted {
    item: item.into(),
    command: "mcp__Orchester__edit_file README.md".into(),
  };
  let ended = |item: &str, status| AgentEvent::CommandFinished {
    item: item.into(),
    command: "mcp__Orchester__edit_file README.md".into(),
    output: "README.md has not been read".into(),
    exit_code: None,
    status,
  };
  let events = vec![
    started("toolu_edit"),
    told("toolu_edit"),
    ended("toolu_edit", CommandStatus::Completed),
    edit("toolu_edit", "README.md"),
    started("toolu_unread"),
    told("toolu_unread"),
    ended("toolu_unread", CommandStatus::Failed),
    told("mcp-1"),
    AgentEvent::McpToolFinished {
      item: "mcp-1".into(),
      server: "Orchester".into(),
      tool: "edit_file".into(),
      status: CommandStatus::Completed,
      asked: false,
    },
    edit("mcp-1", "docs/a.md"),
  ];
  let entries = transcript(execution, events);
  let shown: Vec<(&str, bool)> = entries
    .iter()
    .map(|entry| match &entry.kind {
      EntryKind::Command(command) => ("command", command.native.as_ref() == Some(&asked)),
      EntryKind::McpTool(call) => ("mcp", call.native.as_ref() == Some(&asked)),
      EntryKind::FileChange(_) => ("change", false),
      _ => ("other", false),
    })
    .collect();
  assert_eq!(
    shown,
    [("command", true), ("command", true), ("mcp", true)],
    "each call's row carries what it asked, and no change shows twice"
  );
  let kinds: Vec<Option<ToolKind>> = entries.iter().map(tool_kind).collect();
  assert_eq!(
    kinds,
    [Some(ToolKind::Edit), Some(ToolKind::Command), Some(ToolKind::Edit)]
  );
  let summary = GroupSummary::of(&entries);
  assert_eq!(summary.edits, 1, "README.md, edited by both");
  assert_eq!(
    summary.failed,
    Some(("mcp__Orchester__edit_file README.md".into(), None))
  );
}

/// A Claude Code agent's call of Orchester's `ask_question` is its question card's (C·15), no tool row: while
/// it waits and once answered. A call that failed asked nothing, so it stays; another server's tool of that
/// name is no question.
#[test]
fn timeline_leaves_a_call_of_ask_question_to_its_card() {
  let execution = StepExecutionId::generate();
  let mut events = vec![AgentEvent::CommandStarted {
    item: "toolu_waits".into(),
    command: "mcp__Orchester__ask_question".into(),
  }];
  events.extend(command("toolu_answered", "mcp__Orchester__ask_question", 0));
  events.extend(command("toolu_refused", "mcp__Orchester__ask_question", 1));
  events.extend(command("toolu_other", "mcp__docs__ask_question", 0));
  let shown: Vec<bool> = transcript(execution, events).iter().map(shown_as_card).collect();
  assert_eq!(shown, [true, true, false, false]);
}
