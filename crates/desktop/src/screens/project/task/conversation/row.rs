//! One row of the chat, in its column: a step's divider, a fold of finished steps, the steps that
//! did not start, an agent's message with its chips, a person's message, a card, a problem row, a
//! return or restart, a step's end or result card, or the run's end; in *Tools*, an entry or a
//! message on one line; and last, while an agent works, the working line (`kpnmP`).

use lurq::{
  app::{component::Component, ctx::Ctx},
  components::Column,
  node::{Element, dimension::Dimension},
};

use super::{
  super::activity::{EntryProps, EntryView, LiveFooter, LiveFooterProps},
  Timeline,
  approval_card::{ApprovalCardProps, ApprovalCardView},
  cards::{self, PlanCardProps, PlanCardView},
  column, divider,
  message::{self, PersonMessageProps, PersonMessageView},
};
use crate::{
  tasks::{
    timeline::{Item, ItemKind, view::Row},
    transcript::EntryKind,
  },
  theme::ExtraSpacing,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RowProps {
  pub(super) line: Line,
  /// Whether unparsed lines are shown line by line.
  pub(super) raw: bool,
}

/// What a row of the list shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Line {
  Row(Box<Row>),
  /// *Builder is working · last event 2 s ago*, after the last row while an agent works.
  Working(LiveFooterProps),
}

impl RowProps {
  /// The row's key in the list.
  pub(super) fn key(&self) -> &str {
    match &self.line {
      Line::Row(row) => row.key(),
      Line::Working(_) => "conversation-working",
    }
  }
}

impl lurq::app::component::DevtoolsInspectable for RowProps {}

pub(super) struct TimelineRowView;

impl Component for TimelineRowView {
  type Props = RowProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RowProps>().clone();
    let timeline = ctx
      .use_context::<Timeline>()
      .expect("the tab provides its timeline before its rows");
    let row = match &props.line {
      Line::Row(row) => &**row,
      Line::Working(working) => {
        return column(ctx.mount_keyed::<LiveFooter>("conversation-working", working.clone()));
      }
    };
    let content = match row {
      Row::Step {
        head,
        key,
        emphasized,
        foldable,
        matches,
      } => spaced(divider::step_divider(
        ctx,
        &timeline,
        (head, key),
        (*emphasized, *foldable, *matches),
      )),
      Row::Folded { key, steps } => spaced(divider::folded(ctx, &timeline, key, steps)),
      Row::NotStarted { steps, .. } => spaced(divider::not_started(ctx, steps)),
      Row::Agent(agent) => spaced(message::agent_row(ctx, agent, &timeline, props.raw)),
      Row::Item(item) => spaced(item_row(ctx, item, &timeline)),
      Row::Entry { key, entry } => ctx.mount_keyed::<EntryView>(
        key,
        EntryProps {
          key: key.clone(),
          task: timeline.task,
          entry: entry.clone(),
          agent: "".into(),
          raw: props.raw,
          time_column: true,
          toggles: timeline.toggles.clone(),
        },
      ),
      Row::Line(item) => message::one_line(ctx, item),
    };
    column(content)
  }
}

/// A person's message, a card, a problem, a return or restart, a step's end or the run's.
fn item_row(ctx: &mut Ctx, item: &Item, timeline: &Timeline) -> Element {
  match &item.kind {
    ItemKind::Message(entry) => ctx.mount_keyed::<PersonMessageView>(
      &format!("person-{}", entry.id),
      PersonMessageProps {
        entry: entry.clone(),
        repo_root: timeline.repo_root.clone(),
        task: timeline.task,
        run: timeline.run,
      },
    ),
    ItemKind::Question { entry, step } => cards::question(ctx, (entry, step.as_ref()), timeline),
    ItemKind::Plan(card) => ctx.mount_keyed::<PlanCardView>(
      &item.key,
      PlanCardProps {
        card: card.clone(),
        task_title: timeline.task_title.clone(),
      },
    ),
    ItemKind::Approval(card) => {
      ctx.mount_keyed::<ApprovalCardView>(&item.key, ApprovalCardProps { card: card.clone() })
    }
    // A tool step's failing command is the tool step's own message (`FeiRG`).
    ItemKind::Problem(entry) if matches!(entry.kind, EntryKind::ToolCommand(_)) => {
      message::tool_step_failure(ctx, item, entry, timeline)
    }
    ItemKind::Problem(entry) => ctx.mount_keyed::<EntryView>(
      &item.key,
      EntryProps {
        key: item.key.clone(),
        task: timeline.task,
        entry: entry.clone(),
        agent: "".into(),
        raw: false,
        time_column: false,
        toggles: timeline.toggles.clone(),
      },
    ),
    ItemKind::Mark(mark) => divider::mark(ctx, timeline, mark),
    ItemKind::Result(card) if card.ended_on => cards::result(ctx, card, timeline),
    ItemKind::Result(card) => divider::result(ctx, timeline, card, item.at),
    ItemKind::End(end) => divider::end(ctx, end, (timeline.run_number, item.at)),
    ItemKind::AgentMessage { .. } | ItemKind::Step(_) | ItemKind::Tools(_) => unreachable_row(),
  }
}

/// Agent messages, steps and tool groups have rows of their own ([`Row::Agent`], [`Row::Step`]).
fn unreachable_row() -> Element {
  Column::new().into()
}

/// A row's room above and below it: the chat's 20 between rows (`jl9lq`).
fn spaced(content: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(ExtraSpacing::Inline.role())
    .child(content)
    .into()
}
