//! The chat (`Chat` `RM1e7` in `D1PYAe`): the run's timeline as rows 12 apart, 32 in from the sides, in a
//! virtualized list that follows the run (`ui::chat::follow`), opening at its end or at the card that waits for the
//! person: a step's divider, finished steps folded into one line,
//! the steps a failure kept from starting, returns and restarts and the run's end ([`divider`]); an agent's
//! message with its tool groups ([`message`], [`chips`]); the person's message, or the Orchestrator chat's; a card
//! that waits for or records a decision ([`cards`], C3's); a problem row ([`entry`]); a step's result card
//! ([`result`]); after them the answers and messages the Orchestrator chat proposed that no one applied
//! ([`proposal`]); and last, while an agent works, the working line ([`working`]). A person's choice to open or fold a
//! step or a group is kept while the timeline grows ([`Toggles`]).

mod cards;
mod chips;
mod divider;
mod entry;
mod message;
mod proposal;
mod result;
mod working;

use std::{collections::HashMap, sync::Arc};

use lurq::{
  app::{component::Component, ctx::Ctx, theme::SpacingSize},
  components::{Column, Stack, VirtualizedList},
  core::Signal,
  layout::StackAlignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{InvocationId, ProjectId, StepExecutionId},
  project::TaskKey,
  run::{DecidedBy, DecidedToolApproval, PendingToolApproval, ToolApprovalDecision},
};
use orchester_workspace::tasks::{
  conversation::orchestrator::{ChatMerge, Proposal},
  detail::TaskDetail,
  timeline::{
    Item, ItemKind,
    group::unwrapped,
    view::{Row, Shown},
    waits_for_you,
  },
  transcript::{Entry, EntryKind},
};

use self::proposal::{ProposalProps, ProposalRow};
pub(crate) use self::{divider::step_label, message::links};
pub(super) use self::{
  message::person_message,
  proposal::ChatProject,
  working::{Wait, Working},
};
use crate::{
  navigation::{Navigation, link::Open},
  routes::{ProjectPage, Route, TaskView},
  theme::ExtraSpacing,
  ui::chat::{
    follow::{self, Follow},
    names::ProjectNames,
  },
};

/// What the chat's rows share: whose timeline it is, and what a person opened or folded in it.
#[derive(Clone)]
pub(super) struct Timeline {
  pub(super) project: ProjectId,
  /// The project as the Orchestrator chat's proposals are decided in it.
  pub(super) chat_project: ChatProject,
  /// The projects' names, for what a link to another project's place says.
  pub(super) names: ProjectNames,
  pub(super) key: TaskKey,
  /// The task, for the names and numbers of its run's steps.
  pub(super) detail: Arc<TaskDetail>,
  pub(super) toggles: Toggles,
  /// The run's invocations whose agent works now: only their unfinished calls run. While no agent works
  /// (the run waits, paused or was interrupted), it is empty, and an unfinished call has no end.
  pub(super) working: Arc<[InvocationId]>,
  /// The run's tool requests that wait for the person, and those decided.
  pub(super) pending: Arc<Vec<PendingToolApproval>>,
  pub(super) decided: Arc<Vec<DecidedToolApproval>>,
  /// The Orchestrator chat's merge of the run, when it carried one out (ORC-69).
  pub(super) merged_by: Option<ChatMerge>,
}

impl Timeline {
  /// Whether a request of `invocation` to run `command` (its first line, as a group names it) waits for the
  /// person: the command has not started.
  fn awaits_approval(&self, invocation: InvocationId, command: &str) -> bool {
    self.pending.iter().any(|request| {
      request.invocation == invocation
        && request.command
        && unwrapped(&request.action).lines().next().unwrap_or_default() == command
    })
  }

  /// How a person or automation answered `invocation`'s request to run `command`, when one denied it.
  fn denied_by(&self, invocation: InvocationId, command: &str) -> Option<DecidedBy> {
    self
      .decided
      .iter()
      .filter(|decided| decided.request.invocation == invocation && decided.request.command)
      .filter(|decided| decided.decision == Some(ToolApprovalDecision::Denied))
      .find(|decided| unwrapped(&decided.request.action) == unwrapped(command))
      .and_then(|decided| decided.decided_by)
  }

  /// Whether invocation `invocation` works now.
  fn works(&self, invocation: InvocationId) -> bool {
    self.working.contains(&invocation)
  }

  fn of(ctx: &mut Ctx) -> Self {
    ctx
      .use_context::<Self>()
      .expect("the Conversation tab provides its timeline before its rows")
  }

  /// What opens `view` of the task where a press asks (`navigation::link`).
  fn open(&self, ctx: &mut Ctx, view: TaskView) -> Open {
    let page = ProjectPage::task(self.key.clone(), view, self.detail.not_shared());
    Navigation::of(ctx).to(Route::Project(self.project, page))
  }

  /// "03 Implement", the step `execution` executed, while the run's pipeline has it.
  fn step_of(&self, ctx: &Ctx, execution: Option<StepExecutionId>) -> Option<Arc<str>> {
    let step = self.detail.run.as_ref()?.step_of(execution?)?;
    Some(step_label(ctx, step.number, &step.name))
  }

  /// The run's number in the project.
  fn run_number(&self) -> u32 {
    self.detail.run.as_ref().map_or(0, |run| run.run.number().get())
  }
}

/// Rows a person opened or folded, by row key: a step, a tool group, a tool row with output.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct OpenRows(HashMap<Arc<str>, bool>);

impl lurq::app::component::DevtoolsInspectable for OpenRows {}

/// Open or folded per row key, where a person changed how the row starts.
#[derive(Clone)]
pub(super) struct Toggles {
  rows: Signal<OpenRows>,
  /// A row the person opens or folds stays where it is: the chat stops following.
  follow: Follow,
}

impl Toggles {
  pub(super) fn new(ctx: &mut Ctx, follow: Follow) -> Self {
    Self {
      rows: ctx.signal(OpenRows::default()),
      follow,
    }
  }

  pub(super) fn is_open(&self, key: &str, default: bool) -> bool {
    self.rows.get().0.get(key).copied().unwrap_or(default)
  }

  /// Opens every row of `keys`, as *Show them* does for the steps of a fold.
  fn open_all(&self, keys: Vec<Arc<str>>) {
    self.follow.hold_in_place();
    self.rows.update(move |rows| {
      for key in keys {
        rows.0.insert(key, true);
      }
    });
  }

  fn flip(&self, key: &Arc<str>, default: bool) {
    self.follow.hold_in_place();
    let key = key.clone();
    self.rows.update(move |rows| {
      let open = rows.0.get(&key).copied().unwrap_or(default);
      rows.0.insert(key, !open);
    });
  }
}

/// What a row of the list shows.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Line {
  Row(Box<Row>),
  /// An answer or a message the Orchestrator chat proposed for the run and no one applied ([`proposal`]).
  Proposal(Box<Proposal>),
  /// *Builder is working · last event 2 s ago*, after the last row while an agent works.
  Working(Working),
  /// Room under the last row while *Jump to latest* floats over the chat's foot, so the row can scroll clear
  /// of it.
  JumpRoom,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RowProps {
  line: Line,
  /// The row's key in the list.
  key: Arc<str>,
}

impl lurq::app::component::DevtoolsInspectable for RowProps {}

impl RowProps {
  fn of(line: Line) -> Self {
    let key = match &line {
      Line::Row(row) => row.key().clone(),
      Line::Proposal(proposal) => proposal.key(),
      Line::Working(_) => "conversation-working".into(),
      Line::JumpRoom => "conversation-jump-room".into(),
    };
    Self { line, key }
  }

  /// The row's key in the list.
  fn key(&self) -> &str {
    &self.key
  }
}

/// The chat of `shown`, the rows of the timeline `items`, then the Orchestrator chat's `proposals` for the run, then
/// `working`, the working line while an agent works; *Jump to latest* under it while it does not follow the run, with
/// what arrived since (`arrived` counts the transcript's entries, the conversation's messages and the proposals so
/// far).
pub(super) fn chat(
  ctx: &mut Ctx,
  follow: &Follow,
  (shown, items, proposals): (&Shown, &[Item], &[Proposal]),
  (working, arrived): (Option<Working>, usize),
) -> Element {
  // Until the person scrolls, the first card that waits for them opens the chat instead of its end.
  let waiting = items
    .iter()
    .find(|item| waits_for_you(item))
    .map(|item| item.key.to_string());
  let reveal = follow.follow(shown.rows.len(), arrived, waiting);
  let rows: Arc<Vec<RowProps>> = Arc::new(
    shown
      .rows
      .iter()
      .map(|row| Line::Row(Box::new(row.clone())))
      .chain(
        proposals
          .iter()
          .map(|proposal| Line::Proposal(Box::new(proposal.clone()))),
      )
      .chain(working.map(Line::Working))
      .chain((!follow.following.get()).then_some(Line::JumpRoom))
      .map(RowProps::of)
      .collect(),
  );
  let list = follow.list(VirtualizedList::new_shared(ctx, rows), arrived, reveal);
  let list = list
    .list_key("conversation-timeline")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .mount_keyed::<RowView, _, _, _>(|row| row.key().to_string(), Clone::clone);
  // Floats over the chat's foot (`Lqg8j` in `B67XdS`): it takes no room from the rows.
  let jump = (!follow.following.get()).then(|| follow.jump(ctx, arrived));
  let rows = Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .padding_top(SpacingSize::Sm)
    .padding_bottom(SpacingSize::Xs)
    .child(list)
    .child(follow.end(ctx, "conversation-follow-end"));
  Stack::new()
    .id("conversation-chat")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .stack_align(StackAlignment::BottomCenter)
    .child(rows)
    .with_children(jump)
    .into()
}

/// `shown` without the tool groups that only report a turn's token usage: the step's divider carries its
/// tokens. An agent's row left with neither a message nor a group goes too.
pub(super) fn without_usage(mut shown: Shown) -> Shown {
  for row in &mut shown.rows {
    if let Row::Agent(agent) = row {
      agent.chips.retain(|chip| !only_usage(&chip.group.entries));
    }
  }
  shown
    .rows
    .retain(|row| !matches!(row, Row::Agent(agent) if agent.text.is_none() && agent.chips.is_empty()));
  shown
}

/// Whether `entries` only report token usage.
fn only_usage(entries: &[Entry]) -> bool {
  entries.iter().all(|entry| matches!(entry.kind, EntryKind::Usage(_)))
}

struct RowView;

impl Component for RowView {
  type Props = RowProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RowProps>().clone();
    let id = format!("conversation-row:{}", props.key());
    let content = match &props.line {
      Line::Working(working) => ctx.mount_keyed::<working::WorkingLine>("conversation-working-line", working.clone()),
      Line::JumpRoom => follow::jump_room(),
      Line::Row(row) => row_content(ctx, row),
      Line::Proposal(proposal) => {
        let props = ProposalProps {
          project: Timeline::of(ctx).chat_project,
          proposal: (**proposal).clone(),
        };
        ctx.mount_keyed::<ProposalRow>(&props.proposal.key(), props)
      }
    };
    // Half the chat's 12 between rows above and under each (`gap` of `RM1e7`).
    Column::new()
      .id(id)
      .width(Dimension::Pct(100.0))
      .padding_horizontal(SpacingSize::Section)
      .padding_vertical(ExtraSpacing::Snug.role())
      .child(content)
  }
}

fn row_content(ctx: &mut Ctx, row: &Row) -> Element {
  let timeline = Timeline::of(ctx);
  match row {
    Row::Step {
      head, key, foldable, ..
    } => divider::step(ctx, &timeline, (head, key), *foldable),
    Row::Folded { key, steps } => divider::folded(ctx, &timeline, key, steps),
    Row::NotStarted { steps, .. } => divider::not_started(ctx, steps),
    Row::Agent(agent) => message::agent_row(ctx, &timeline, agent),
    Row::Item(item) | Row::Line(item) => item_row(ctx, &timeline, item),
    Row::Entry { key, entry } => entry::tool_row(ctx, &timeline, key, entry),
  }
}

/// A person's message, a card, a problem, a return or restart, a step's result or the run's end.
fn item_row(ctx: &mut Ctx, timeline: &Timeline, item: &Item) -> Element {
  match &item.kind {
    ItemKind::Message(entry) => person_message(ctx, (timeline.project, entry)),
    ItemKind::Question { .. } | ItemKind::Plan(_) | ItemKind::Approval(_) | ItemKind::Choice(_) => {
      cards::card(ctx, timeline, item)
    }
    ItemKind::Problem(entry) => entry::problem_row(ctx, timeline, &item.key, entry),
    ItemKind::Mark(mark) => divider::mark(ctx, timeline, mark),
    ItemKind::BaseMoved(mark) => divider::base_moved(ctx, &item.key, mark),
    ItemKind::Result(card) => result::card(ctx, timeline, card),
    ItemKind::End(end) => divider::end(ctx, timeline, end, item.at),
    // An agent's message, a step and a tool group are rows of their own (`Row::Agent`, `Row::Step`).
    ItemKind::AgentMessage { .. } | ItemKind::Step(_) | ItemKind::Tools(_) => Column::new().into(),
  }
}
