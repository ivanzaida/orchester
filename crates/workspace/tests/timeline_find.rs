//! Finding a word in a run's timeline (ORC-306, `Task page · redesign` 13f `aoVlE`): plain text with case and accents
//! ignored, in messages, commands, paths and output, in the order the chat shows them, each place naming the item, the
//! step whose fold hides it and the call and line it is in; a search a newer one superseded stops.

use std::sync::Arc;

use orchester_core::{
  connection::stream::{AgentEvent, CommandStatus, FileChange, FileChangeKind},
  id::{InvocationId, StepExecutionId},
  run::{ActivityEntry, StepStatus, TokenUsage},
  time::Timestamp,
};
use orchester_workspace::tasks::{
  timeline::{
    Item, ItemKind, StepHead, ToolGroup,
    find::{Hit, MAX_HITS, Needle, Spot, find, fold},
  },
  transcript::{Entry, entries},
};

/// Messages read as written: these tests search no Markdown.
fn as_written(_: &str) -> Option<String> {
  None
}

/// Every place `query` is in `items`.
fn found(items: &[Item], query: &str) -> Vec<Hit> {
  let needle = Needle::new(query).expect("a query");
  find(items, &needle, &as_written, &|| true).expect("not superseded")
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

fn command(item: &str, line: &str, output: &str) -> [AgentEvent; 2] {
  [
    AgentEvent::CommandStarted {
      item: item.into(),
      command: line.into(),
    },
    AgentEvent::CommandFinished {
      item: item.into(),
      command: line.into(),
      output: output.into(),
      exit_code: Some(0),
      status: CommandStatus::Completed,
    },
  ]
}

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

fn said(text: &str) -> ItemKind {
  ItemKind::AgentMessage {
    agent: "Builder".into(),
    step: "Implement".into(),
    text: text.into(),
  }
}

/// A step that wrote a message and ran a group: a read, a command with output, an edit.
fn implement() -> Vec<Item> {
  let execution = StepExecutionId::generate();
  let mut tools = Vec::new();
  tools.extend(command(
    "1",
    "rg -n \"Retry-After\" src/",
    "src/sender.rs:9: Retry-After",
  ));
  tools.extend(command(
    "2",
    "cargo test -p webhooks",
    "running 48 tests\nWARN sender: connection reset by peer\ntest result: ok",
  ));
  tools.push(AgentEvent::FileChanged {
    item: "3".into(),
    changes: vec![FileChange {
      path: "src/webhooks/retry.rs".into(),
      kind: FileChangeKind::Modified,
    }],
  });
  vec![
    item(
      "step-implement",
      execution,
      ItemKind::Step(head(execution, 3, "Implement")),
    ),
    item(
      "message",
      execution,
      said("I'll honour Retry-After and cap the retry at 60 s; retry stops on 410."),
    ),
    item(
      "tools",
      execution,
      ItemKind::Tools(ToolGroup::new(transcript(execution, tools))),
    ),
  ]
}

#[test]
fn timeline_find_ignores_case_and_accents() {
  assert_eq!(fold('É'), 'e');
  assert_eq!(fold('ł'), 'l');
  assert_eq!(fold('Ё'), 'е');
  let needle = Needle::new("  cafe ").expect("a query");
  assert_eq!(needle.places("Café, CAFÉ and café"), vec![0..5, 7..12, 17..22]);
  assert!(Needle::new("   ").is_none(), "a blank query looks for nothing");
}

#[test]
fn timeline_find_counts_every_place_in_the_order_the_chat_shows_them() {
  let items = implement();
  let hits = found(&items, "retry");
  let spots: Vec<(&str, Spot)> = hits.iter().map(|hit| (&*hit.item, hit.spot)).collect();
  assert_eq!(
    spots,
    [
      ("message", Spot::Said),
      ("message", Spot::Said),
      ("message", Spot::Said),
      ("tools", Spot::Title),
      ("tools", Spot::Output(0)),
      ("tools", Spot::Title),
    ],
    "the message's three, the command's line and output, then the edited path"
  );
  assert!(
    hits.iter().all(|hit| hit.step.as_deref() == Some("step-implement")),
    "each names the step whose fold hides it"
  );
  let calls: Vec<usize> = hits[3..]
    .iter()
    .map(|hit| hit.call.as_ref().expect("a call").rank)
    .collect();
  assert_eq!(calls, [0, 0, 2], "the read, then the edit after the test command");
}

#[test]
fn timeline_find_names_the_line_of_the_output() {
  let items = implement();
  let hits = found(&items, "connection reset");
  assert_eq!(hits.len(), 1);
  let hit = &hits[0];
  assert_eq!(hit.spot, Spot::Output(1), "the output's second line");
  assert_eq!(hit.range, 13..29, "the bytes of the place in that line");
  let call = hit.call.as_ref().expect("in a call");
  assert_eq!(call.rank, 1);
  assert!(call.key.starts_with("entry-"), "the call's row key: {}", call.key);
}

#[test]
fn timeline_find_keeps_a_place_through_a_read_again() {
  let items = implement();
  let before = found(&items, "retry");
  let mut grown = items.clone();
  grown.push(item(
    "later",
    StepExecutionId::generate(),
    said("One more retry before the end."),
  ));
  let after = found(&grown, "retry");
  assert_eq!(after.len(), before.len() + 1, "a new place adds to the count");
  assert!(
    before.iter().zip(&after).all(|(old, new)| old.same_place(new)),
    "the places found before stay where they were"
  );
}

#[test]
fn timeline_find_stops_when_a_newer_search_supersedes_it() {
  let items = implement();
  let needle = Needle::new("retry").expect("a query");
  assert_eq!(find(&items, &needle, &as_written, &|| false), None);
}

#[test]
fn timeline_find_stops_counting_at_its_most() {
  let execution = StepExecutionId::generate();
  let text: Arc<str> = "a ".repeat(MAX_HITS + 10).into();
  let items = vec![item("many", execution, said(&text))];
  assert_eq!(found(&items, "a").len(), MAX_HITS);
}
