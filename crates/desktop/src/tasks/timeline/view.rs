//! What the timeline shows of a run (`docs/design/orchester-workspace.md`, "Filters and search"):
//! *All*, *Messages* (messages, cards, problems, returns and restarts, step headers and results; tool
//! groups hidden and counted) or *Tools* (every tool event as a flat transcript, messages as
//! one-line entries, returns and restarts as they are); one step execution or all; and only what
//! matches a search, a group with a match opened on its matching entries. A step folds to its header unless it runs,
//! waits for the person, is the one picked or has a match; a person's choice to open or close a step or a group wins.

use std::sync::Arc;

use orchester_core::id::StepExecutionId;

use super::{Item, ItemKind, StepHead, ToolGroup, mark::MarkKind};
use crate::tasks::transcript::{Entry, EntryKind};

/// The *Show* control.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Show {
  #[default]
  All,
  Messages,
  Tools,
}

/// What the person chose to see.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Choice {
  pub show: Show,
  /// The one step execution shown; `None` for all.
  pub step: Option<StepExecutionId>,
  /// What to search for; blank for no search.
  pub query: String,
}

impl Choice {
  fn needle(&self) -> Option<String> {
    let query = self.query.trim();
    (!query.is_empty()).then(|| query.to_lowercase())
  }
}

/// One row of the timeline as shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
  Step {
    head: StepHead,
    key: Arc<str>,
    open: bool,
  },
  /// A message, a card, a problem, a result or the run's end.
  Item(Item),
  /// A tool group: `shown` are the entries it shows while open (with a search, the matching ones),
  /// `left_out` the ones a search hides.
  Group {
    key: Arc<str>,
    group: ToolGroup,
    open: bool,
    shown: Vec<Entry>,
    left_out: Vec<Entry>,
  },
  /// A transcript entry on its own line, in *Tools*.
  Entry {
    key: Arc<str>,
    entry: Entry,
  },
  /// A message on one line, in *Tools*.
  Line(Item),
}

impl Row {
  pub fn key(&self) -> &Arc<str> {
    match self {
      Self::Step { key, .. } | Self::Group { key, .. } | Self::Entry { key, .. } => key,
      Self::Item(item) | Self::Line(item) => &item.key,
    }
  }
}

/// The rows and what a filter or a search left out.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Shown {
  pub rows: Vec<Row>,
  /// Matches of the search: items and group entries.
  pub matches: usize,
  /// The steps with a match, in order.
  pub matched_steps: Vec<StepHead>,
  /// The steps another step's filter hides.
  pub hidden_steps: Vec<StepHead>,
  /// Tool calls and groups *Messages* hides.
  pub hidden_tools: (usize, usize),
}

/// What `items` show for `choice`; `open(key, default)` says whether a step or a group is open.
pub fn shown(items: &[Item], choice: &Choice, open: &dyn Fn(&str, bool) -> bool) -> Shown {
  let needle = choice.needle();
  let mut shown = Shown::default();
  let in_step = |item: &Item| choice.step.is_none_or(|step| item.execution == Some(step));
  let mut step_open = true;
  for (index, item) in items.iter().enumerate() {
    if let ItemKind::Mark(_) = &item.kind {
      // A mark comes before the header of its step, so that step's fold does not hide it.
      if in_step(item) {
        row(&mut shown, item, (choice.show, needle.as_deref()), open);
      }
      continue;
    }
    if let ItemKind::Step(head) = &item.kind {
      // A step that never ran has nothing to show but its header.
      if head.execution.is_none() {
        if choice.step.is_none() && needle.is_none() {
          shown.rows.push(Row::Step {
            head: head.clone(),
            key: item.key.clone(),
            open: false,
          });
        }
        continue;
      }
      if !in_step(item) {
        shown.hidden_steps.push(head.clone());
        step_open = false;
        continue;
      }
      let matched = needle
        .as_deref()
        .map(|needle| own_items(items, index).any(|item| matches(item, needle)));
      if matched == Some(false) {
        step_open = false;
        continue;
      }
      if matched == Some(true) {
        shown.matched_steps.push(head.clone());
      }
      let default = head.starts_open() || choice.step.is_some() || matched == Some(true);
      let key = item.key.clone();
      step_open = open(&key, default) || needle.is_some();
      shown.rows.push(Row::Step {
        head: head.clone(),
        key,
        open: step_open,
      });
      continue;
    }
    if !in_step(item) || (item.execution.is_some() && !step_open) {
      continue;
    }
    row(&mut shown, item, (choice.show, needle.as_deref()), open);
  }
  shown
}

/// The items after the step header at `index`, up to the next header or the mark before it.
fn own_items(items: &[Item], index: usize) -> impl Iterator<Item = &Item> {
  items[index + 1..]
    .iter()
    .take_while(|item| !matches!(item.kind, ItemKind::Step(_) | ItemKind::Mark(_)))
}

fn row(shown: &mut Shown, item: &Item, (show, needle): (Show, Option<&str>), open: &dyn Fn(&str, bool) -> bool) {
  if let ItemKind::Tools(group) = &item.kind {
    return tools(shown, item, group, (show, needle), open);
  }
  if let Some(needle) = needle {
    if !matches(item, needle) {
      return;
    }
    shown.matches += 1;
  }
  match (show, &item.kind) {
    (Show::Tools, ItemKind::AgentMessage { .. } | ItemKind::Message(_) | ItemKind::Question { .. }) => {
      shown.rows.push(Row::Line(item.clone()));
    }
    (Show::Tools, ItemKind::Problem(entry)) => shown.rows.push(Row::Entry {
      key: item.key.clone(),
      entry: entry.clone(),
    }),
    // The Tools view lists returns and restarts as well (02.B7 `zyusO`).
    (Show::Tools, ItemKind::Mark(_)) => shown.rows.push(Row::Item(item.clone())),
    (Show::Tools, _) => {}
    (Show::All | Show::Messages, _) => shown.rows.push(Row::Item(item.clone())),
  }
}

fn tools(
  shown: &mut Shown,
  item: &Item,
  group: &ToolGroup,
  (show, needle): (Show, Option<&str>),
  open: &dyn Fn(&str, bool) -> bool,
) {
  let (matching, left_out): (Vec<Entry>, Vec<Entry>) = match needle {
    Some(needle) => group
      .entries
      .iter()
      .cloned()
      .partition(|entry| entry_matches(entry, needle)),
    None => (group.entries.clone(), Vec::new()),
  };
  if needle.is_some() && matching.is_empty() {
    return;
  }
  // Messages hides the group, so its matches are not among what the view shows.
  if needle.is_some() && show != Show::Messages {
    shown.matches += matching.len();
  }
  match show {
    Show::Messages => {
      shown.hidden_tools.0 += group.summary.calls;
      shown.hidden_tools.1 += 1;
    }
    Show::Tools => shown.rows.extend(matching.into_iter().map(|entry| Row::Entry {
      key: format!("entry-{}-{}", entry.sequence, entry.part).into(),
      entry,
    })),
    Show::All => shown.rows.push(Row::Group {
      key: item.key.clone(),
      group: group.clone(),
      open: needle.is_some() || open(&item.key, false),
      shown: matching,
      left_out,
    }),
  }
}

/// Whether what `item` says contains `needle`, in lower case.
fn matches(item: &Item, needle: &str) -> bool {
  let found = |text: &str| text.to_lowercase().contains(needle);
  match &item.kind {
    ItemKind::AgentMessage { text, .. } => found(text),
    ItemKind::Message(entry) | ItemKind::Question { entry, .. } => found(&entry.body),
    ItemKind::Plan(card) => found(&card.plan.text),
    ItemKind::Approval(card) => [
      Some(card.request.action.as_str()),
      Some(card.request.server.as_str()),
      card.request.target.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(found),
    ItemKind::Problem(entry) => entry_matches(entry, needle),
    ItemKind::Result(card) => card.reported.as_deref().is_some_and(found),
    ItemKind::Tools(group) => group.entries.iter().any(|entry| entry_matches(entry, needle)),
    ItemKind::Mark(mark) => match &mark.kind {
      MarkKind::ReturnedWithNote { note, .. } => found(note),
      MarkKind::Returned { .. } | MarkKind::Restarted { .. } => false,
    },
    ItemKind::Step(_) | ItemKind::End(_) => false,
  }
}

/// Whether a transcript entry's message, command, path or output contains `needle`.
fn entry_matches(entry: &Entry, needle: &str) -> bool {
  let found = |text: &str| text.to_lowercase().contains(needle);
  match &entry.kind {
    EntryKind::Message(text) | EntryKind::Reasoning(text) | EntryKind::Error(text) | EntryKind::NotSignedIn(text) => {
      found(text)
    }
    EntryKind::Command(command) => found(&command.command) || found(&command.output),
    EntryKind::ToolCommand(command) => found(&command.command) || found(&command.output),
    EntryKind::FileChange(change) => found(&change.path),
    EntryKind::Raw(lines) => lines.iter().any(|(_, line)| found(line)),
    EntryKind::McpTool(call) => found(&call.server) || found(&call.tool),
    EntryKind::McpServer(server, _) => server.as_deref().is_some_and(found),
    EntryKind::ExemptionNotApplied { server, tool, .. } => found(server) || found(tool),
    EntryKind::Usage(_) | EntryKind::Problem(_) => false,
  }
}
