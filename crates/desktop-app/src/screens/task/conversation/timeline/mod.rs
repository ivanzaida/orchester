//! The chat (`Chat` `RM1e7` in `D1PYAe`): the run's timeline as rows 12 apart, 32 in from the sides, in a
//! virtualized list that follows the run (`ui::chat::follow`), opening at its end or at the card that waits for the
//! person: a step's divider, finished steps folded into one line,
//! the steps a failure kept from starting, returns and restarts and the run's end ([`divider`]); an agent's
//! message with its tool groups ([`message`], [`chips`]); the person's message, or the Orchestrator chat's; a card
//! that waits for or records a decision ([`cards`], C3's), or the quiet row of a request the run's approval mode
//! approved, those of Autopilot grouped per step ([`approvals`]); a problem row ([`entry`]); a step's result card
//! ([`result`]); what the merge queue did with the run, among them by time ([`delivery`]); after them the answers and
//! messages the Orchestrator chat proposed that no one applied ([`proposal`]); and last, while an agent works, the
//! working line ([`working`]). A person's choice to open or fold a step or a group is kept while the timeline grows
//! ([`Toggles`]).

mod approvals;
mod cards;
mod chips;
mod delivery;
mod divider;
mod entry;
mod message;
mod model;
mod permission;
mod proposal;
mod result;
mod service;
mod switched;
mod todo;
mod working;

use std::{
  collections::{HashMap, HashSet},
  sync::Arc,
};

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
use orchester_workspace::{
  files::Version,
  tasks::{
    conversation::orchestrator::{ChatMerge, Proposal},
    detail::TaskDetail,
    timeline::{
      Item, ItemKind,
      delivery::DeliveryLine,
      group::unwrapped,
      view::{Row, Shown},
      waits_for_you,
    },
    transcript::{Entry, EntryKind},
  },
};

use self::proposal::{ProposalProps, ProposalRow};
pub(crate) use self::{divider::step_label, message::links};
pub(super) use self::{
  message::person_message,
  permission::rule_text,
  proposal::ChatProject,
  working::{Wait, Working},
};
use super::{
  search::marks::{Marks, row_mark},
  shared::Shared,
};
use crate::{
  navigation::{Navigation, link::Open},
  project::ShownProject,
  routes::{ProjectPage, Route, TaskView},
  theme::ExtraSpacing,
  ui::chat::{
    follow::{self, Follow},
    names::ProjectNames,
  },
};

/// What the chat's rows share: whose timeline it is, and what a person opened or folded in it. The tab shares it
/// (`super::shared`), so a row renders again when it changed.
#[derive(Clone, PartialEq)]
pub(super) struct Timeline {
  pub(super) project: ProjectId,
  /// The project as the window shows it, whose run changes move the context meter of the step at work.
  pub(super) shown_project: ShownProject,
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
  /// The run goes on, so the lists of its steps at work are pinned over the composer, not on their dividers.
  pub(super) pins: bool,
  /// What the open search marks.
  pub(super) search: Option<Marks>,
  /// The run's pull request with its CI, for its lines among the rows (ORC-335).
  pub(super) ci: Option<crate::screens::task::ci::timeline::CiShare>,
}

impl lurq::app::component::DevtoolsInspectable for Timeline {}

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
    Shared::of(ctx).expect("the Conversation tab shares its timeline before its rows")
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

  /// Who sent message `id` as a Fix CI, if one did (ORC-335).
  fn fixed_by(&self, id: orchester_core::id::MessageId) -> Option<orchester_core::pull_request::ci::fix::FixedBy> {
    use orchester_core::pull_request::ci::fix::FixEventKind;
    self
      .detail
      .run
      .as_ref()?
      .ci_fixes
      .events
      .iter()
      .find_map(|event| match event.kind {
        FixEventKind::Sent { message, by } if message == id => Some(by),
        _ => None,
      })
  }

  /// Where the files a person's message mentions are: the run's worktree, or the project's checkout before a run.
  fn files_at(&self) -> Version {
    self
      .detail
      .run
      .as_ref()
      .map_or(Version::Checkout, |run| Version::Run(run.run.id()))
  }
}

/// Rows a person opened or folded, by row key: a step, a tool group, a tool row with output; and the rows a search
/// opened to show its current match, which fold again when the search closes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct OpenRows {
  chosen: HashMap<Arc<str>, bool>,
  searched: HashSet<Arc<str>>,
}

impl lurq::app::component::DevtoolsInspectable for OpenRows {}

impl OpenRows {
  pub(super) fn is_open(&self, key: &str, default: bool) -> bool {
    self.searched.contains(key) || self.chosen.get(key).copied().unwrap_or(default)
  }

  /// Whether the search opened the row `key`.
  pub(super) fn opened_by_search(&self, key: &str) -> bool {
    self.searched.contains(key)
  }
}

/// Open or folded per row key, where a person changed how the row starts.
#[derive(Clone)]
pub(super) struct Toggles {
  rows: Signal<OpenRows>,
  /// A row the person opens or folds stays where it is: the chat stops following.
  follow: Follow,
}

/// The tab's own handles, the same for as long as the tab is mounted; what they hold is read through them.
impl PartialEq for Toggles {
  fn eq(&self, other: &Self) -> bool {
    self.rows.id() == other.rows.id() && self.follow == other.follow
  }
}

impl Toggles {
  pub(super) fn new(ctx: &mut Ctx, follow: Follow) -> Self {
    Self {
      rows: ctx.signal(OpenRows::default()),
      follow,
    }
  }

  pub(super) fn is_open(&self, key: &str, default: bool) -> bool {
    self.rows.get().is_open(key, default)
  }

  /// Whether the search opened the row `key`, to show its current match.
  pub(super) fn opened_by_search(&self, key: &str) -> bool {
    self.rows.get().opened_by_search(key)
  }

  /// What the person opened or folded so far; reading it in a render re-renders it when they open or fold a row.
  pub(super) fn open_rows(&self) -> OpenRows {
    self.rows.get()
  }

  /// Opens every row of `keys`, as *Show them* does for the steps of a fold.
  fn open_all(&self, keys: Vec<Arc<str>>) {
    self.follow.hold_in_place();
    self.rows.update(move |rows| {
      for key in keys {
        rows.chosen.insert(key, true);
      }
    });
  }

  /// Opens the rows of `keys` for the search, until it closes ([`Toggles::fold_searched`]).
  pub(super) fn open_for_search(&self, keys: Vec<Arc<str>>) {
    if keys.iter().all(|key| self.rows.get_untracked().searched.contains(key)) {
      return;
    }
    self.rows.update(move |rows| rows.searched.extend(keys));
  }

  /// Folds again what the search opened; the chat stays where it is.
  pub(super) fn fold_searched(&self) {
    if self.rows.get_untracked().searched.is_empty() {
      return;
    }
    self.follow.hold_in_place();
    self.rows.update(|rows| rows.searched.clear());
  }

  /// A press opens or folds the row; one the search opened becomes the person's to fold.
  fn flip(&self, key: &Arc<str>, default: bool) {
    self.follow.hold_in_place();
    let key = key.clone();
    self.rows.update(move |rows| {
      let open = rows.is_open(&key, default);
      rows.searched.remove(&key);
      rows.chosen.insert(key, !open);
    });
  }
}

/// What a row of the list shows.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Line {
  /// At the run's start, while Autopilot answers its requests ([`divider::autopilot`]).
  Autopilot,
  /// Shared with the tab's built timeline (`super::built`), so a render of the list copies none of it.
  Row(Arc<Row>),
  /// What the merge queue did with the run ([`delivery`]), shared with the built timeline too.
  Delivery(Arc<DeliveryLine>),
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
      Line::Autopilot => "conversation-autopilot".into(),
      Line::Row(row) => row.key().clone(),
      Line::Delivery(line) => line.key.clone(),
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

/// A line of what the merge queue did with the run, and the index of the row it goes before (`super::built`).
pub(super) type Placed = (usize, Arc<DeliveryLine>);

/// `rows` with the merge queue's lines among them, each before the row its index names.
fn with_delivery(rows: &[Arc<Row>], delivery: &[Placed]) -> Vec<Line> {
  let mut lines = Vec::with_capacity(rows.len() + delivery.len());
  let mut queued = delivery.iter().peekable();
  for (index, row) in rows.iter().enumerate() {
    while let Some((_, line)) = queued.next_if(|(before, _)| *before <= index) {
      lines.push(Line::Delivery(line.clone()));
    }
    lines.push(Line::Row(row.clone()));
  }
  lines.extend(queued.map(|(_, line)| Line::Delivery(line.clone())));
  lines
}

/// The chat of `rows` with the merge queue's lines (`delivery`) among them, what the timeline `items` show, then the
/// Orchestrator chat's `proposals` for the run, then `working`, the working line while an agent works; *Jump to latest*
/// under it while it does not follow the run, with what arrived since (`arrived` counts the transcript's entries, the
/// conversation's messages and the proposals so far; the queue's lines count too). `autopilot`, whether Autopilot
/// answers the run's requests, leads the rows with its line. While the search is open, the row of its current place
/// (`found`) is brought into view, and its `track` of marks floats over the chat's end.
pub(super) fn chat(
  ctx: &mut Ctx,
  follow: &Follow,
  (rows, delivery, autopilot): (&[Arc<Row>], &[Placed], bool),
  (items, proposals): (&[Item], &[Proposal]),
  (working, arrived): (Option<Working>, usize),
  (found, track): (Option<String>, Option<Element>),
) -> Element {
  let arrived = arrived + delivery.len();
  // Until the person scrolls, the first card that waits for them opens the chat instead of its end.
  let waiting = items
    .iter()
    .find(|item| waits_for_you(item))
    .map(|item| item.key.to_string());
  let lead = usize::from(autopilot);
  let reveal = follow.follow(rows.len() + delivery.len() + lead, arrived, waiting);
  let reveal = found.or(reveal);
  let rows: Arc<Vec<RowProps>> = Arc::new(
    autopilot
      .then_some(Line::Autopilot)
      .into_iter()
      .chain(with_delivery(rows, delivery))
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
  let rows: Element = match track {
    Some(track) => Stack::with(StackAlignment::TopEnd, [rows.into(), track])
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .into(),
    None => rows.into(),
  };
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
      Line::Autopilot => {
        let timeline = Timeline::of(ctx);
        divider::autopilot(ctx, &timeline)
      }
      Line::Row(row) => row_content(ctx, row),
      Line::Delivery(line) => {
        let timeline = Timeline::of(ctx);
        delivery::line(ctx, &timeline, line)
      }
      Line::Proposal(proposal) => {
        let props = ProposalProps {
          project: Timeline::of(ctx).chat_project,
          proposal: (**proposal).clone(),
        };
        ctx.mount_keyed::<ProposalRow>(&props.proposal.key(), props)
      }
    };
    let mark = match &props.line {
      Line::Row(row) => Timeline::of(ctx).search.and_then(|marks| marks.row(&on_row(row))),
      _ => None,
    };
    // Half the chat's 12 between rows above and under each (`gap` of `RM1e7`).
    let row = Column::new()
      .id(id)
      .width(Dimension::Pct(100.0))
      .padding_horizontal(SpacingSize::Section)
      .padding_vertical(ExtraSpacing::Snug.role())
      .child(content);
    row_mark(row, mark)
  }
}

/// The items whose words `row` shows that a search marks on the row: a card, a problem, a notice. A message, an
/// agent's, a person's or a note the work returned with, fills behind its matched words instead.
fn on_row(row: &Row) -> Vec<&str> {
  match row {
    Row::Item(item) | Row::Line(item) if !is_message(item) => vec![&*item.key],
    Row::Approvals { items, .. } => items.iter().map(|item| &*item.key).collect(),
    Row::Item(_)
    | Row::Line(_)
    | Row::Agent(_)
    | Row::Step { .. }
    | Row::Folded { .. }
    | Row::NotStarted { .. }
    | Row::Entry { .. } => Vec::new(),
  }
}

/// Whether `item` is a message, its words drawn as one text: a person's, or the note a return came with.
const fn is_message(item: &Item) -> bool {
  matches!(item.kind, ItemKind::Message(_) | ItemKind::Mark(_))
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
    Row::Approvals { key, items, open } => approvals::rows(ctx, &timeline, (key, items), *open),
    Row::Entry { key, entry } => entry::tool_row(ctx, &timeline, key, entry),
  }
}

/// A person's message, a card, a problem, a return or restart, the run's switch to Manual, a change of its
/// permissions, a step's result or the run's end. A message fills behind what the search found in it.
fn item_row(ctx: &mut Ctx, timeline: &Timeline, item: &Item) -> Element {
  let found = || timeline.search.as_ref().and_then(|marks| marks.in_said(&item.key));
  match &item.kind {
    ItemKind::Message(entry) => person_message(
      ctx,
      (
        (timeline.project, &timeline.chat_project.root, &timeline.files_at()),
        entry,
      ),
      (found().as_ref(), timeline.fixed_by(entry.id)),
    ),
    ItemKind::Question { .. } | ItemKind::Plan(_) | ItemKind::Approval(_) | ItemKind::Choice(_) => {
      cards::card(ctx, timeline, item)
    }
    ItemKind::Problem(entry) => service::card(ctx, timeline, &item.key, entry)
      .unwrap_or_else(|| entry::problem_row(ctx, timeline, &item.key, entry)),
    ItemKind::Mark(mark) => divider::mark(ctx, timeline, mark, found().as_ref()),
    ItemKind::BaseMoved(mark) => divider::base_moved(ctx, &item.key, mark),
    ItemKind::SwitchedToManual(switched) => switched::row(ctx, switched, item.at),
    ItemKind::Permission(line) => permission::row(ctx, timeline, (&item.key, line, item.at)),
    ItemKind::Model(line) => model::row(ctx, timeline, (&item.key, line, item.at)),
    ItemKind::BackToProfile(back) => model::back_row(ctx, timeline, &item.key, back),
    ItemKind::Result(card) => result::card(ctx, timeline, card),
    ItemKind::End(end) => divider::end(ctx, timeline, end, item.at),
    // An agent's message, a step and a tool group are rows of their own (`Row::Agent`, `Row::Step`).
    ItemKind::AgentMessage { .. } | ItemKind::Step(_) | ItemKind::Tools(_) => Column::new().into(),
  }
}
