//! The Conversation tab's timeline as data (ORC-61): which transcript entries are tool events and
//! which kind, the command a CLI wrapped in its shell, what a tool group's summary counts, and what
//! *Show*, the step filter, a search and the folds leave of the timeline.

use std::sync::Arc;

use orchester_core::{
  connection::stream::{AgentEvent, CommandStatus, FileChange, FileChangeKind},
  conversation::Arrival,
  id::{InvocationId, MessageId, RunId, StepExecutionId},
  run::{ActivityEntry, PendingToolApproval, StepStatus, TokenUsage},
  time::Timestamp,
};
use orchester_desktop::tasks::{
  conversation::{Author, Entry as MessageEntry, EntryState},
  timeline::{
    ApprovalCard, Item, ItemKind, StepHead, ToolGroup,
    group::{GroupSummary, ToolKind, tool_kind, unwrapped},
    interleaved,
    mark::{Mark, MarkKind},
    view::{Choice, Row, Show, shown},
  },
  transcript::{Entry, entries},
};

fn at(seconds: i64) -> Timestamp {
  Timestamp::from_unix_millis(seconds * 1_000)
}

/// The transcript entries of `events`, of one execution, a second apart.
fn transcript(execution: StepExecutionId, events: Vec<AgentEvent>) -> Vec<Entry> {
  let invocation = InvocationId::generate();
  let stored: Vec<ActivityEntry> = events
    .into_iter()
    .zip(1..)
    .map(|(event, sequence)| ActivityEntry {
      sequence,
      execution,
      invocation,
      at: at(i64::try_from(sequence).expect("small")),
      event,
    })
    .collect();
  entries(&stored)
}

fn command(item: &str, text: &str, exit: i64) -> [AgentEvent; 2] {
  [
    AgentEvent::CommandStarted {
      item: item.into(),
      command: text.into(),
    },
    AgentEvent::CommandFinished {
      item: item.into(),
      command: text.into(),
      output: format!("output of {text}"),
      exit_code: Some(exit),
      status: if exit == 0 {
        CommandStatus::Completed
      } else {
        CommandStatus::Failed
      },
    },
  ]
}

fn edit(item: &str, path: &str) -> AgentEvent {
  AgentEvent::FileChanged {
    item: item.into(),
    changes: vec![FileChange {
      path: path.into(),
      kind: FileChangeKind::Modified,
    }],
  }
}

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

/// A step's header that ran and ended.
fn head(execution: StepExecutionId, number: usize, step: &str) -> StepHead {
  StepHead {
    execution: Some(execution),
    number,
    step: step.into(),
    agent: "Builder".into(),
    command: false,
    provider: None,
    vendor: None,
    model: "gpt-5.5".into(),
    invocations: 1,
    started: Some(at(1)),
    ended: Some(at(60)),
    usage: TokenUsage::default(),
    before_edit: None,
    status: Some(StepStatus::Passed),
    attempt: None,
    ended_on: false,
  }
}

fn item(key: &str, execution: StepExecutionId, kind: ItemKind) -> Item {
  Item {
    key: key.into(),
    execution: Some(execution),
    at: at(2),
    kind,
  }
}

/// Two steps: Research with a message and a group of a read and a command, Implement with a
/// message and a group with an edit.
fn two_steps() -> (Vec<Item>, StepExecutionId, StepExecutionId) {
  let (research, implement) = (StepExecutionId::generate(), StepExecutionId::generate());
  let message = |text: &str| ItemKind::AgentMessage {
    agent: "Builder".into(),
    step: "Research".into(),
    text: text.into(),
  };
  let mut research_tools = Vec::new();
  research_tools.extend(command("1", "Read src/cancel.ts", 0));
  research_tools.extend(command("2", "pnpm test", 0));
  let items = vec![
    item("step-research", research, ItemKind::Step(head(research, 1, "Research"))),
    item(
      "research-message",
      research,
      message("The cancel dialog is in src/cancel.ts."),
    ),
    item(
      "research-tools",
      research,
      ItemKind::Tools(ToolGroup::new(transcript(research, research_tools))),
    ),
    item(
      "step-implement",
      implement,
      ItemKind::Step(head(implement, 2, "Implement")),
    ),
    item("implement-message", implement, message("Edited the dialog.")),
    item(
      "implement-tools",
      implement,
      ItemKind::Tools(ToolGroup::new(transcript(implement, vec![edit("3", "src/dialog.ts")]))),
    ),
  ];
  (items, research, implement)
}

fn kinds(rows: &[Row]) -> Vec<&str> {
  rows
    .iter()
    .map(|row| match row {
      Row::Step { .. } => "step",
      Row::Item(_) => "item",
      Row::Group { .. } => "group",
      Row::Entry { .. } => "entry",
      Row::Line(_) => "line",
    })
    .collect()
}

#[test]
fn timeline_finished_steps_fold_unless_opened() {
  let (items, ..) = two_steps();
  let folded = shown(&items, &Choice::default(), &|_, default| default);
  assert_eq!(kinds(&folded.rows), ["step", "step"]);
  let opened = shown(&items, &Choice::default(), &|key, default| {
    key == "step-research" || default
  });
  assert_eq!(kinds(&opened.rows), ["step", "item", "group", "step"]);
}

#[test]
fn timeline_show_hides_groups_or_flattens_them() {
  let (items, ..) = two_steps();
  let open = |_: &str, _: bool| true;
  let messages = shown(
    &items,
    &Choice {
      show: Show::Messages,
      ..Choice::default()
    },
    &open,
  );
  assert_eq!(kinds(&messages.rows), ["step", "item", "step", "item"]);
  assert_eq!(messages.hidden_tools, (3, 2), "3 tool calls in 2 groups");
  let tools = shown(
    &items,
    &Choice {
      show: Show::Tools,
      ..Choice::default()
    },
    &open,
  );
  assert_eq!(
    kinds(&tools.rows),
    ["step", "line", "entry", "entry", "step", "line", "entry"]
  );
}

#[test]
fn timeline_search_keeps_matches_and_opens_their_group_on_them() {
  let (items, ..) = two_steps();
  let search = |query: &str| {
    shown(
      &items,
      &Choice {
        query: query.into(),
        ..Choice::default()
      },
      &|_, default| default,
    )
  };
  let cancel = search("CANCEL");
  // The message and the read match; the step without a match hides.
  assert_eq!(cancel.matches, 2);
  assert_eq!(kinds(&cancel.rows), ["step", "item", "group"]);
  let Row::Group {
    open, shown, left_out, ..
  } = &cancel.rows[2]
  else {
    panic!("the group with the match");
  };
  assert!(open, "a match opens its group");
  assert_eq!((shown.len(), left_out.len()), (1, 1));
  assert_eq!(cancel.matched_steps.len(), 1);
  assert_eq!(&*cancel.matched_steps[0].step, "Research");

  let nothing = search("refresh token");
  assert!(nothing.rows.is_empty());
  assert_eq!(nothing.matches, 0);
}

#[test]
fn timeline_step_filter_shows_one_step_open_and_names_the_others() {
  let (items, research, _) = two_steps();
  let only = shown(
    &items,
    &Choice {
      step: Some(research),
      ..Choice::default()
    },
    &|_, default| default,
  );
  assert_eq!(kinds(&only.rows), ["step", "item", "group"], "the step picked is open");
  assert_eq!(only.hidden_steps.len(), 1);
  assert_eq!(&*only.hidden_steps[0].step, "Implement");
}

/// Two commands of one execution, A at 1 s and B at 3 s, and something else at `between`.
fn around(execution: StepExecutionId, between: Item) -> Vec<Item> {
  let mut events = Vec::new();
  events.extend(command("1", "pnpm test", 0));
  events.extend(command("2", "pnpm lint", 0));
  let agent: Arc<str> = "Builder".into();
  let step: Arc<str> = "Implement".into();
  interleaved(
    &transcript(execution, events),
    execution,
    (&agent, &step),
    vec![between],
  )
}

fn at_millis(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

/// A person's message sent now at `at`, delivered mid-turn to `execution`.
fn sent_now(execution: StepExecutionId, at: Timestamp) -> Item {
  Item {
    key: "message".into(),
    execution: Some(execution),
    at,
    kind: ItemKind::Message(MessageEntry {
      id: MessageId::generate(),
      author: Author::You,
      at,
      body: "Also cover the cancelled dialog.".into(),
      state: EntryState::Delivered {
        agent: "Builder".into(),
        at,
        arrival: Arrival::MidTurn,
        sent_now: true,
      },
      execution: Some(execution),
      sent_now: true,
    }),
  }
}

fn kinds_of(items: &[Item]) -> Vec<&str> {
  items
    .iter()
    .map(|item| match &item.kind {
      ItemKind::Tools(_) => "tools",
      ItemKind::Message(_) => "message",
      ItemKind::Approval(_) => "approval",
      _ => "other",
    })
    .collect()
}

#[test]
fn timeline_a_message_between_two_tool_calls_splits_their_group() {
  let execution = StepExecutionId::generate();
  let items = around(execution, sent_now(execution, at_millis(2_500)));
  assert_eq!(kinds_of(&items), ["tools", "message", "tools"]);
  let ItemKind::Tools(first) = &items[0].kind else {
    panic!("the group before the message");
  };
  assert_eq!(first.summary.named, [Arc::<str>::from("pnpm test")]);

  // Of two at the same time, the message comes first.
  let items = around(execution, sent_now(execution, at(3)));
  assert_eq!(kinds_of(&items), ["tools", "message", "tools"]);
}

#[test]
fn timeline_an_approval_card_between_two_tool_calls_splits_their_group() {
  let execution = StepExecutionId::generate();
  let request = PendingToolApproval {
    id: "approval-1".into(),
    run: RunId::generate(),
    execution,
    invocation: InvocationId::generate(),
    agent: "Builder".into(),
    command: true,
    scope: None,
    server: "Codex".into(),
    action: "pnpm lint".into(),
    target: None,
    requested_at: at_millis(2_500),
    native: None,
  };
  let card = Item {
    key: "approval-1".into(),
    execution: Some(execution),
    at: request.requested_at,
    kind: ItemKind::Approval(ApprovalCard {
      request,
      step: "Implement".into(),
      decided: None,
    }),
  };
  assert_eq!(kinds_of(&around(execution, card)), ["tools", "approval", "tools"]);
}

#[test]
fn timeline_tools_view_keeps_a_message_between_the_tool_calls_around_it() {
  let execution = StepExecutionId::generate();
  let mut items = vec![item("step", execution, ItemKind::Step(head(execution, 1, "Implement")))];
  items.extend(around(execution, sent_now(execution, at_millis(2_500))));
  let tools = shown(
    &items,
    &Choice {
      show: Show::Tools,
      ..Choice::default()
    },
    &|_, _| true,
  );
  assert_eq!(kinds(&tools.rows), ["step", "entry", "line", "entry"]);
}

#[test]
fn timeline_a_declined_memory_call_is_a_problem_row() {
  let execution = StepExecutionId::generate();
  let declined = transcript(
    execution,
    vec![AgentEvent::McpToolDeclined {
      item: "1".into(),
      server: "orchester-memory".into(),
      tool: "memory_search".into(),
    }],
  );
  assert_eq!(declined.iter().map(tool_kind).collect::<Vec<_>>(), [None]);
}

#[test]
fn timeline_messages_view_counts_only_the_matches_it_shows() {
  let (items, ..) = two_steps();
  let messages = shown(
    &items,
    &Choice {
      show: Show::Messages,
      query: "cancel".into(),
      ..Choice::default()
    },
    &|_, default| default,
  );
  // The message matches; the read in the hidden group matches too, but is not shown.
  assert_eq!(messages.matches, 1);
  assert_eq!(messages.hidden_tools, (2, 1));
}

#[test]
fn timeline_a_return_shows_before_its_step_in_every_view_even_folded() {
  let (mut items, _, implement) = two_steps();
  let mark = Mark {
    attempt: 2,
    at: Some(at(2)),
    by_person: false,
    kind: MarkKind::Restarted {
      step: "Implement".into(),
    },
  };
  items.insert(3, item("mark-2", implement, ItemKind::Mark(mark)));
  let folded = shown(&items, &Choice::default(), &|_, default| default);
  assert_eq!(kinds(&folded.rows), ["step", "item", "step"]);
  for show in [Show::Messages, Show::Tools] {
    let rows = shown(
      &items,
      &Choice {
        show,
        ..Choice::default()
      },
      &|_, default| default,
    );
    assert_eq!(kinds(&rows.rows), ["step", "item", "step"], "{show:?}");
  }
  let other = shown(
    &items,
    &Choice {
      step: Some(items[0].execution.expect("research")),
      ..Choice::default()
    },
    &|_, default| default,
  );
  assert!(
    other.rows.iter().all(|row| &**row.key() != "mark-2"),
    "another step's filter hides the mark"
  );
}

#[test]
fn timeline_a_tool_step_s_failing_command_is_a_row_of_its_own() {
  use orchester_core::connection::stream::ToolCommandResult;
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
