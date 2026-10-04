//! What the timeline shows of a run as a chat (`docs/design/orchester-workspace.md`, "Decided: C with
//! the rail on the right"): *All*, *Messages* (messages, cards, problems, returns and restarts, step
//! dividers and results; tool groups hidden and counted) or *Tools* (every tool event as a flat
//! transcript, messages as one-line entries, returns and restarts as they are); one step or all; and
//! only what matches a search, a group with a match opened on its matching entries.
//!
//! In *All* an agent's message carries the chips of the tool groups after it ([`AgentRow`]); tool
//! activity with no message before it is a row of the agent's own. A step starts with its divider.
//! It shows its items unless it is folded: steps that finished fold, and the finished ones in a row
//! share one line ([`Row::Folded`]); the step that runs, waits for the person, failed or the run
//! ended on stays open, and so does the one before the last that ran (`kpnmP`, `K4AOD`, `FeiRG`). A
//! person's choice to open or fold a step or a chip wins, but the step picked and a step with a
//! match always show their items.

mod matching;

use std::sync::Arc;

use orchester_core::{id::StepExecutionId, run::StepStatus, time::Timestamp};

use self::matching::{entry_matches, matches};
use super::{Item, ItemKind, StepHead, ToolGroup};
use crate::tasks::transcript::Entry;

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
  /// The step executions shown, those of the step picked in the rail; `None` for all.
  pub step: Option<Vec<StepExecutionId>>,
  /// What to search for; blank for no search.
  pub query: String,
}

impl Choice {
  fn needle(&self) -> Option<String> {
    let query = self.query.trim();
    (!query.is_empty()).then(|| query.to_lowercase())
  }

  /// Whether the step filter keeps `item`.
  fn keeps(&self, item: &Item) -> bool {
    self
      .step
      .as_ref()
      .is_none_or(|picked| item.execution.is_some_and(|execution| picked.contains(&execution)))
  }
}

/// One row of the timeline as shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
  /// A step's divider, its items after it; `emphasized` while it runs, waits, failed, ended the run or
  /// is the one picked; `foldable` once it finished, unless it is picked or searched; `matches`
  /// while searching.
  Step {
    head: StepHead,
    key: Arc<str>,
    emphasized: bool,
    foldable: bool,
    matches: Option<usize>,
  },
  /// Finished steps in a row, folded into one line: their keys and headers.
  Folded {
    key: Arc<str>,
    steps: Vec<(Arc<str>, StepHead)>,
  },
  /// The steps that did not start after a failure, as one muted divider.
  NotStarted { key: Arc<str>, steps: Vec<StepHead> },
  /// An agent's message with the chips of its tool groups.
  Agent(AgentRow),
  /// A person's message, a card, a problem, a return or restart, a result or the run's end.
  Item(Item),
  /// A transcript entry on its own line, in *Tools*.
  Entry { key: Arc<str>, entry: Entry },
  /// A message on one line, in *Tools*.
  Line(Item),
}

impl Row {
  pub fn key(&self) -> &Arc<str> {
    match self {
      Self::Step { key, .. }
      | Self::Folded { key, .. }
      | Self::NotStarted { key, .. }
      | Self::Entry { key, .. }
      | Self::Agent(AgentRow { key, .. }) => key,
      Self::Item(item) | Self::Line(item) => &item.key,
    }
  }
}

/// An agent's chat row (`V17jfc` as `kpnmP` draws it): who, when, what it wrote and a chip per tool
/// group after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentRow {
  pub key: Arc<str>,
  pub execution: Option<StepExecutionId>,
  /// The agent role, or the tool's name for a tool step.
  pub agent: Arc<str>,
  /// A command tool runs the step: the row is the tool's.
  pub tool_step: bool,
  /// The step it names beside its time: the first row of a step whose divider is muted (`o0KDAq`).
  pub step: Option<Arc<str>>,
  pub at: Timestamp,
  /// What the agent wrote; `None` for tool activity with no message before it.
  pub text: Option<Arc<str>>,
  pub chips: Vec<Chip>,
}

/// A tool group as a chip: `shown` are the entries it lists while open (with a search, the matching
/// ones), `left_out` the ones a search hides.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chip {
  pub key: Arc<str>,
  pub group: ToolGroup,
  pub open: bool,
  pub shown: Vec<Entry>,
  pub left_out: Vec<Entry>,
}

/// The rows and what a filter or a search left out.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Shown {
  pub rows: Vec<Row>,
  /// Matches of the search: items and group entries.
  pub matches: usize,
  /// The steps with a match, in order.
  pub matched_steps: Vec<StepHead>,
  /// The matches of each step execution shown, while searching.
  pub step_matches: Vec<(StepExecutionId, usize)>,
  /// Tool calls and groups *Messages* hides.
  pub hidden_tools: (usize, usize),
}

/// What `items` show for `choice`; `open(key, default)` says whether a step or a chip is open.
pub fn shown(items: &[Item], choice: &Choice, open: &dyn Fn(&str, bool) -> bool) -> Shown {
  let ran: Vec<StepExecutionId> = items
    .iter()
    .filter_map(|item| match &item.kind {
      ItemKind::Step(head) => head.execution,
      _ => None,
    })
    .collect();
  let mut pass = Pass {
    choice,
    needle: choice.needle(),
    open,
    recent: ran.len().checked_sub(2).map(|index| ran[index]),
    shown: Shown::default(),
    step: None,
    folded: Vec::new(),
    not_started: Vec::new(),
  };
  for (index, item) in items.iter().enumerate() {
    pass.item(items, index, item);
  }
  pass.close_step();
  pass.flush();
  pass.shown
}

/// The step whose items follow, shown open.
struct OpenStep {
  head: StepHead,
  /// Its divider's row.
  row: usize,
  matches: usize,
  /// The step its first agent row names, when its divider is muted.
  name: Option<Arc<str>>,
}

/// One pass over the items.
struct Pass<'a> {
  choice: &'a Choice,
  needle: Option<String>,
  open: &'a dyn Fn(&str, bool) -> bool,
  /// The execution before the last that ran, which starts open as well.
  recent: Option<StepExecutionId>,
  shown: Shown,
  step: Option<OpenStep>,
  /// Finished steps folded since the last row shown.
  folded: Vec<(Arc<str>, StepHead)>,
  not_started: Vec<StepHead>,
}

impl Pass<'_> {
  fn item(&mut self, items: &[Item], index: usize, item: &Item) {
    match &item.kind {
      // A mark comes before the divider of its step, so that step's fold does not hide it.
      ItemKind::Mark(_) => {
        self.close_step();
        if self.choice.keeps(item) {
          self.flush();
          self.row(item);
        }
      }
      ItemKind::Step(head) if head.execution.is_none() => {
        self.close_step();
        if self.choice.step.is_none() && self.needle.is_none() {
          self.not_started.push(head.clone());
        }
      }
      ItemKind::Step(head) => self.step(items, index, item, head),
      _ if item.execution.is_none() => {
        self.close_step();
        if self.choice.step.is_none() {
          self.flush();
          self.row(item);
        }
      }
      _ if self.step.is_some() && self.choice.keeps(item) => self.row(item),
      _ => {}
    }
  }

  fn step(&mut self, items: &[Item], index: usize, item: &Item, head: &StepHead) {
    self.close_step();
    if !self.choice.keeps(item) {
      return;
    }
    let matched = self.needle.as_deref().map(|needle| {
      items[index + 1..]
        .iter()
        .take_while(|item| !matches!(item.kind, ItemKind::Step(_) | ItemKind::Mark(_)))
        .any(|item| matches(item, needle))
    });
    if matched == Some(false) {
      return;
    }
    if matched == Some(true) {
      self.shown.matched_steps.push(head.clone());
    }
    let picked = self.choice.step.is_some();
    let default = head.starts_open() || picked || matched == Some(true) || head.execution == self.recent;
    // The step picked and a step with a match show their items whatever the person folded.
    if !(picked || self.needle.is_some() || (self.open)(&item.key, default)) {
      self.folded.push((item.key.clone(), head.clone()));
      return;
    }
    self.flush();
    let emphasized = head.starts_open() || picked;
    let finished = head.status == Some(StepStatus::Passed) && head.ended.is_some();
    self.shown.rows.push(Row::Step {
      head: head.clone(),
      key: item.key.clone(),
      emphasized,
      foldable: finished && !picked && self.needle.is_none(),
      matches: None,
    });
    self.step = Some(OpenStep {
      head: head.clone(),
      row: self.shown.rows.len() - 1,
      matches: 0,
      name: (!emphasized).then(|| head.step.clone()),
    });
  }

  /// Ends the open step: while searching, its divider says how many matches it has.
  fn close_step(&mut self) {
    let Some(open) = self.step.take() else {
      return;
    };
    if self.needle.is_none() {
      return;
    }
    if let Some(Row::Step { matches, .. }) = self.shown.rows.get_mut(open.row) {
      *matches = Some(open.matches);
    }
    if let Some(execution) = open.head.execution {
      self.shown.step_matches.push((execution, open.matches));
    }
  }

  /// Shows the folded steps and the steps that did not start gathered so far.
  fn flush(&mut self) {
    if let Some((first, _)) = self.folded.first() {
      let key = format!("fold-{first}").into();
      let steps = std::mem::take(&mut self.folded);
      self.shown.rows.push(Row::Folded { key, steps });
    }
    if !self.not_started.is_empty() {
      let steps = std::mem::take(&mut self.not_started);
      self.shown.rows.push(Row::NotStarted {
        key: "not-started".into(),
        steps,
      });
    }
  }

  fn count(&mut self, matches: usize) {
    self.shown.matches += matches;
    if let Some(open) = &mut self.step {
      open.matches += matches;
    }
  }

  fn row(&mut self, item: &Item) {
    if let ItemKind::Tools(group) = &item.kind {
      return self.tools(item, group);
    }
    if let Some(needle) = self.needle.as_deref() {
      if !matches(item, needle) {
        return;
      }
      self.count(1);
    }
    let row = match (self.choice.show, &item.kind) {
      (Show::Tools, ItemKind::AgentMessage { .. } | ItemKind::Message(_) | ItemKind::Question { .. }) => {
        Row::Line(item.clone())
      }
      (Show::Tools, ItemKind::Problem(entry)) => Row::Entry {
        key: item.key.clone(),
        entry: entry.clone(),
      },
      // The Tools view lists returns and restarts as well (02.B7 `zyusO`).
      (Show::Tools, ItemKind::Mark(_)) => Row::Item(item.clone()),
      (Show::Tools, _) => return,
      (Show::All | Show::Messages, ItemKind::AgentMessage { agent, text, .. }) => {
        Row::Agent(self.agent_row(item, agent.clone(), Some(text.clone())))
      }
      (Show::All | Show::Messages, _) => Row::Item(item.clone()),
    };
    self.shown.rows.push(row);
  }

  fn agent_row(&mut self, item: &Item, agent: Arc<str>, text: Option<Arc<str>>) -> AgentRow {
    let (tool_step, step) = self
      .step
      .as_mut()
      .map_or((false, None), |open| (open.head.command, open.name.take()));
    AgentRow {
      key: item.key.clone(),
      execution: item.execution,
      agent,
      tool_step,
      step,
      at: item.at,
      text,
      chips: Vec::new(),
    }
  }

  fn tools(&mut self, item: &Item, group: &ToolGroup) {
    let (matching, left_out): (Vec<Entry>, Vec<Entry>) = match self.needle.as_deref() {
      Some(needle) => group
        .entries
        .iter()
        .cloned()
        .partition(|entry| entry_matches(entry, needle)),
      None => (group.entries.clone(), Vec::new()),
    };
    let searching = self.needle.is_some();
    if searching && matching.is_empty() {
      return;
    }
    // Messages hides the group, so its matches are not among what the view shows.
    if searching && self.choice.show != Show::Messages {
      self.count(matching.len());
    }
    match self.choice.show {
      Show::Messages => {
        self.shown.hidden_tools.0 += group.summary.calls;
        self.shown.hidden_tools.1 += 1;
      }
      Show::Tools => self.shown.rows.extend(matching.into_iter().map(|entry| Row::Entry {
        key: format!("entry-{}-{}", entry.sequence, entry.part).into(),
        entry,
      })),
      Show::All => {
        let chip = Chip {
          key: item.key.clone(),
          group: group.clone(),
          open: searching || (self.open)(&item.key, false),
          shown: matching,
          left_out,
        };
        self.chip(item, chip);
      }
    }
  }

  /// Puts `chip` in the agent's row right before it, or in a row of its own.
  fn chip(&mut self, item: &Item, chip: Chip) {
    if let Some(Row::Agent(row)) = self.shown.rows.last_mut()
      && row.execution == item.execution
    {
      row.chips.push(chip);
      return;
    }
    let agent = self
      .step
      .as_ref()
      .map_or_else(|| "".into(), |open| open.head.agent.clone());
    let mut row = self.agent_row(item, agent, None);
    row.chips.push(chip);
    self.shown.rows.push(Row::Agent(row));
  }
}
