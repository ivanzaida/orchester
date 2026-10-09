//! The Conversation tab's timeline as data (ORC-61, as a chat since the rail on the right): its tool
//! events ([`tool_events`]), how a command's end reads ([`command_end`]), and what *Show*, the step picked in the
//! rail, a search and the folds leave of the timeline.

mod command_end;
mod tool_events;

use std::sync::Arc;

use orchester_core::{
  connection::stream::{AgentEvent, CommandStatus, FileChange, FileChangeKind},
  conversation::Arrival,
  id::{InvocationId, MessageId, RunId, StepExecutionId},
  run::{ActivityEntry, PendingToolApproval, StepStatus, TokenUsage},
  time::Timestamp,
};
use orchester_workspace::tasks::{
  conversation::{Author, Entry as MessageEntry, EntryState},
  timeline::{
    ApprovalCard, Item, ItemKind, StepHead, ToolGroup,
    group::tool_kind,
    interleaved,
    mark::{Mark, MarkKind},
    view::{AgentRow, Chip, Choice, Row, Show, shown},
  },
  transcript::{Entry, entries},
};

/// Messages read as written: these tests search no Markdown.
fn as_written(_: &str) -> Option<String> {
  None
}

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
    approved: false,
    tool_run: None,
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
      Row::Folded { .. } => "folded",
      Row::NotStarted { .. } => "not-started",
      Row::Agent(AgentRow { text: Some(_), .. }) => "agent",
      Row::Agent(AgentRow { text: None, .. }) => "tools",
      Row::Item(_) => "item",
      Row::Entry { .. } => "entry",
      Row::Line(_) => "line",
    })
    .collect()
}

/// The chips of the agent's row at `index`.
fn chips(rows: &[Row], index: usize) -> &[Chip] {
  match &rows[index] {
    Row::Agent(row) => &row.chips,
    row => panic!("an agent's row, not {row:?}"),
  }
}

#[test]
fn timeline_a_message_carries_the_chips_of_its_tool_groups() {
  let (items, ..) = two_steps();
  let opened = shown(&items, &Choice::default(), &|_, _| true, &as_written);
  assert_eq!(kinds(&opened.rows), ["step", "agent", "step", "agent"]);
  let research = chips(&opened.rows, 1);
  assert_eq!(research.len(), 1, "one chip per group");
  assert!(research[0].open, "a person opened it");
  let closed = shown(
    &items,
    &Choice::default(),
    &|key, _| key.starts_with("step"),
    &as_written,
  );
  assert!(!chips(&closed.rows, 1)[0].open, "a chip starts closed");
  // Tool activity with no message before it is the agent's row of its own.
  let without: Vec<Item> = items
    .iter()
    .filter(|item| !matches!(item.kind, ItemKind::AgentMessage { .. }))
    .cloned()
    .collect();
  let rows = shown(&without, &Choice::default(), &|_, _| true, &as_written).rows;
  assert_eq!(kinds(&rows), ["step", "tools", "step", "tools"]);
  let Row::Agent(row) = &rows[1] else {
    panic!("the agent's row");
  };
  assert_eq!(&*row.agent, "Builder");
}

#[test]
fn timeline_finished_steps_fold_into_one_line_unless_opened() {
  let (mut items, ..) = two_steps();
  let review = StepExecutionId::generate();
  items.push(item("step-review", review, ItemKind::Step(head(review, 3, "Review"))));
  items.push(item(
    "review-message",
    review,
    ItemKind::AgentMessage {
      agent: "Builder".into(),
      step: "Review".into(),
      text: "The dialog closes.".into(),
    },
  ));
  // The last two that ran stay open, also between two steps, before the next starts; a finished step
  // before them folds.
  let folded = shown(&items, &Choice::default(), &|_, default| default, &as_written);
  assert_eq!(kinds(&folded.rows), ["folded", "step", "agent", "step", "agent"]);
  // Finished steps in a row share one line, with their keys to open them.
  let all = shown(&items, &Choice::default(), &|_, _| false, &as_written);
  assert_eq!(kinds(&all.rows), ["folded"]);
  let Row::Folded { steps, .. } = &all.rows[0] else {
    panic!("the fold");
  };
  let keys: Vec<&str> = steps.iter().map(|(key, _)| &**key).collect();
  assert_eq!(keys, ["step-research", "step-implement", "step-review"]);
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
    &as_written,
  );
  assert_eq!(kinds(&messages.rows), ["step", "agent", "step", "agent"]);
  assert!(chips(&messages.rows, 1).is_empty(), "Messages hides the groups");
  assert_eq!(messages.hidden_tools, (3, 2), "3 tool calls in 2 groups");
  let tools = shown(
    &items,
    &Choice {
      show: Show::Tools,
      ..Choice::default()
    },
    &open,
    &as_written,
  );
  assert_eq!(
    kinds(&tools.rows),
    ["step", "line", "entry", "entry", "step", "line", "entry"]
  );
}

#[test]
fn timeline_search_keeps_matches_opens_their_chip_on_them_and_counts_them_by_step() {
  let (items, research, _) = two_steps();
  let search = |query: &str| {
    shown(
      &items,
      &Choice {
        query: query.into(),
        ..Choice::default()
      },
      &|_, default| default,
      &as_written,
    )
  };
  let cancel = search("CANCEL");
  // The message and the read match; the step without a match hides.
  assert_eq!(cancel.matches, 2);
  assert_eq!(kinds(&cancel.rows), ["step", "agent"]);
  let Row::Step { matches, .. } = &cancel.rows[0] else {
    panic!("the step's divider");
  };
  assert_eq!(*matches, Some(2), "its divider counts its matches");
  assert_eq!(cancel.step_matches, [(research, 2)]);
  let chip = &chips(&cancel.rows, 1)[0];
  assert!(chip.open, "a match opens its chip");
  assert_eq!((chip.shown.len(), chip.left_out.len()), (1, 1));
  assert_eq!(cancel.matched_steps.len(), 1);
  assert_eq!(&*cancel.matched_steps[0].step, "Research");

  let nothing = search("refresh token");
  assert!(nothing.rows.is_empty());
  assert_eq!(nothing.matches, 0);
}

#[test]
fn timeline_step_filter_shows_the_step_picked_open() {
  let (items, research, _) = two_steps();
  let only = shown(
    &items,
    &Choice {
      step: Some(vec![research]),
      ..Choice::default()
    },
    &|_, _| false,
    &as_written,
  );
  assert_eq!(kinds(&only.rows), ["step", "agent"], "the step picked is open");
  let Row::Step { emphasized, .. } = &only.rows[0] else {
    panic!("the step's divider");
  };
  assert!(emphasized, "the step picked is in $text");
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
      decided_by: None,
      review: None,
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
    &as_written,
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
    &as_written,
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
  for show in [Show::All, Show::Messages, Show::Tools] {
    let rows = shown(
      &items,
      &Choice {
        show,
        ..Choice::default()
      },
      &|_, _| false,
      &as_written,
    );
    assert_eq!(kinds(&rows.rows), ["folded", "item", "folded"], "{show:?}");
  }
  let other = shown(
    &items,
    &Choice {
      step: Some(vec![items[0].execution.expect("research")]),
      ..Choice::default()
    },
    &|_, default| default,
    &as_written,
  );
  assert!(
    other.rows.iter().all(|row| &**row.key() != "mark-2"),
    "another step's filter hides the mark"
  );
}

#[test]
fn timeline_search_finds_a_question_with_its_step() {
  let execution = StepExecutionId::generate();
  let asked = at(2);
  let question = item(
    "question",
    execution,
    ItemKind::Question {
      entry: MessageEntry {
        id: MessageId::generate(),
        author: Author::Agent("Builder".into()),
        at: asked,
        body: "Keep email sign-in?".into(),
        state: EntryState::Question { open: true },
        execution: Some(execution),
        sent_now: false,
      },
      step: Some("Implement".into()),
    },
  );
  let items = vec![
    item("step", execution, ItemKind::Step(head(execution, 1, "Implement"))),
    question,
  ];
  let found = shown(
    &items,
    &Choice {
      query: "sign-in".into(),
      ..Choice::default()
    },
    &|_, default| default,
    &as_written,
  );
  assert_eq!(found.matches, 1);
  assert!(
    matches!(&found.rows[1], Row::Item(Item { kind: ItemKind::Question { step, .. }, .. }) if step.as_deref() == Some("Implement"))
  );
}

/// A search finds what an agent wrote as the chat shows it, by the app's rendering of its Markdown,
/// and as written.
#[test]
fn timeline_search_finds_a_message_as_rendered() {
  let execution = StepExecutionId::generate();
  let message = item(
    "message",
    execution,
    ItemKind::AgentMessage {
      agent: "Builder".into(),
      step: "Research".into(),
      text: "Use **email** sign-in".into(),
    },
  );
  let items = vec![
    item("step", execution, ItemKind::Step(head(execution, 1, "Research"))),
    message,
  ];
  let search = |query: &str, rendered: &dyn Fn(&str) -> Option<String>| {
    shown(
      &items,
      &Choice {
        query: query.into(),
        ..Choice::default()
      },
      &|_, default| default,
      rendered,
    )
    .matches
  };
  let bold = |text: &str| Some(text.replace("**", ""));
  assert_eq!(search("email sign-in", &bold), 1);
  assert_eq!(search("email sign-in", &as_written), 0);
  assert_eq!(search("**email**", &as_written), 1);
}
