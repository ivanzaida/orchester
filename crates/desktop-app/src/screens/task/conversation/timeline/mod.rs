//! The chat (`Chat` `RM1e7` in `D1PYAe`): the run's timeline as rows 12 apart, 32 in from the sides, in a
//! virtualized list that follows the run ([`follow`]): a step's divider, finished steps folded into one line,
//! the steps a failure kept from starting, returns and restarts and the run's end ([`divider`]); an agent's
//! message with its tool groups ([`message`], [`chips`]); the person's message; a card that waits for or
//! records a decision ([`cards`], C3's); a problem row ([`entry`]); a step's result card ([`result`]); and
//! last, while an agent works, the working line ([`working`]). A person's choice to open or fold a step or a
//! group is kept while the timeline grows ([`Toggles`]).

mod cards;
mod chips;
mod divider;
mod entry;
mod follow;
mod message;
mod result;
mod working;

use std::{collections::HashMap, sync::Arc};

use lurq::{
  app::{component::Component, ctx::Ctx, theme::SpacingSize},
  components::{Column, VirtualizedList},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{InvocationId, ProjectId, StepExecutionId},
  project::TaskKey,
};
use orchester_workspace::tasks::{
  detail::TaskDetail,
  timeline::{
    Item, ItemKind,
    view::{Row, Shown},
  },
  transcript::{Entry, EntryKind},
};

pub(super) use self::{divider::step_label, follow::Follow, message::person_message, working::Working};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskView},
  theme::ExtraSpacing,
};

/// What the chat's rows share: whose timeline it is, and what a person opened or folded in it.
#[derive(Clone)]
pub(super) struct Timeline {
  pub(super) project: ProjectId,
  pub(super) key: TaskKey,
  /// The task, for the names and numbers of its run's steps.
  pub(super) detail: Arc<TaskDetail>,
  pub(super) toggles: Toggles,
  /// The run's invocations whose agent works now: only their unfinished calls run. While no agent works
  /// (the run waits, paused or was interrupted), it is empty, and an unfinished call has no end.
  pub(super) working: Arc<[InvocationId]>,
}

impl Timeline {
  /// Whether invocation `invocation` works now.
  fn works(&self, invocation: InvocationId) -> bool {
    self.working.contains(&invocation)
  }

  fn of(ctx: &mut Ctx) -> Self {
    ctx
      .use_context::<Self>()
      .expect("the Conversation tab provides its timeline before its rows")
  }

  /// Goes to `view` of the task, in the active tab.
  fn open(&self, ctx: &mut Ctx, view: TaskView) -> impl Fn() + Send + Sync + 'static {
    let navigation = Navigation::of(ctx);
    let route = Route::Project(self.project, ProjectPage::Task(self.key.clone(), view));
    move || navigation.go(route.clone())
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
pub(super) struct Toggles(Signal<OpenRows>);

impl Toggles {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self(ctx.signal(OpenRows::default()))
  }

  pub(super) fn is_open(&self, key: &str, default: bool) -> bool {
    self.0.get().0.get(key).copied().unwrap_or(default)
  }

  /// Opens every row of `keys`, as *Show them* does for the steps of a fold.
  fn open_all(&self, keys: Vec<Arc<str>>) {
    self.0.update(move |rows| {
      for key in keys {
        rows.0.insert(key, true);
      }
    });
  }

  fn flip(&self, key: &Arc<str>, default: bool) {
    let key = key.clone();
    self.0.update(move |rows| {
      let open = rows.0.get(&key).copied().unwrap_or(default);
      rows.0.insert(key, !open);
    });
  }
}

/// What a row of the list shows.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Line {
  Row(Box<Row>),
  /// *Builder is working · last event 2 s ago*, after the last row while an agent works.
  Working(Working),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RowProps {
  line: Line,
}

impl lurq::app::component::DevtoolsInspectable for RowProps {}

impl RowProps {
  /// The row's key in the list.
  fn key(&self) -> &str {
    match &self.line {
      Line::Row(row) => row.key(),
      Line::Working(_) => "conversation-working",
    }
  }
}

/// The chat of `shown`, the rows of the timeline `items`, then `working`, the working line while an agent
/// works; *Jump to latest* under it while it does not follow the run, with what arrived since (`arrived`
/// counts the transcript's entries and the conversation's messages so far).
pub(super) fn chat(
  ctx: &mut Ctx,
  follow: &Follow,
  (shown, items): (&Shown, &[Item]),
  (working, arrived): (Option<Working>, usize),
) -> Element {
  let reveal = follow.follow(shown, items, arrived);
  let rows: Arc<Vec<RowProps>> = Arc::new(
    shown
      .rows
      .iter()
      .map(|row| Line::Row(Box::new(row.clone())))
      .chain(working.map(Line::Working))
      .map(|line| RowProps { line })
      .collect(),
  );
  let list = follow.list(VirtualizedList::new_shared(ctx, rows), arrived, reveal);
  let list = list
    .list_key("conversation-timeline")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .mount_keyed::<RowView, _, _, _>(|row| row.key().to_string(), Clone::clone);
  let jump = (!follow.following.get()).then(|| follow.jump(ctx, arrived));
  Column::new()
    .id("conversation-chat")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .padding_top(SpacingSize::Sm)
    .padding_bottom(SpacingSize::Xs)
    .child(list)
    .child(follow.end(ctx))
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
      Line::Row(row) => row_content(ctx, row),
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
    ItemKind::Message(entry) => person_message(ctx, entry),
    ItemKind::Question { .. } | ItemKind::Plan(_) | ItemKind::Approval(_) | ItemKind::Choice(_) => {
      cards::card(ctx, timeline, item)
    }
    ItemKind::Problem(entry) => entry::problem_row(ctx, timeline, &item.key, entry),
    ItemKind::Mark(mark) => divider::mark(ctx, timeline, mark),
    ItemKind::Result(card) => result::card(ctx, timeline, card),
    ItemKind::End(end) => divider::end(ctx, timeline, end, item.at),
    // An agent's message, a step and a tool group are rows of their own (`Row::Agent`, `Row::Step`).
    ItemKind::AgentMessage { .. } | ItemKind::Step(_) | ItemKind::Tools(_) => Column::new().into(),
  }
}
