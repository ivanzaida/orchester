//! One row of the timeline, in its column: a step header, a return or restart before it, a message,
//! a card, a tool group, a problem row, a step's result or the run's end; in *Tools*, an entry or a
//! message on one line; and last, while an agent works, the working line (`RrScB`).

use std::sync::Arc;

use lurq::{
  app::{component::Component, ctx::Ctx, theme::SpacingSize},
  components::Column,
  node::{Element, dimension::Dimension},
};

use super::{
  super::activity::{EntryProps, EntryView, LiveFooter, LiveFooterProps, mark_row, step_header},
  Timeline,
  approval_card::{ApprovalCardProps, ApprovalCardView},
  cards::{self, PlanCardProps, PlanCardView},
  column, group,
  message::{self, PersonMessageProps, PersonMessageView},
};
use crate::tasks::timeline::{Item, ItemKind, view::Row};

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
      Row::Step { head, key, open } => {
        let tokens = head
          .execution
          .map(|execution| Arc::new(timeline.details.open_on(execution)) as Arc<dyn Fn() + Send + Sync>);
        step_header(ctx, (head, key, *open), &timeline.toggles, tokens.as_ref())
      }
      Row::Item(item) => spaced(item_row(ctx, item, &timeline)),
      Row::Group {
        key,
        group,
        open,
        shown,
        left_out,
      } => spaced(group::tool_group(
        ctx,
        &timeline,
        (key, group, *open),
        (shown, left_out),
        props.raw,
      )),
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

/// A message, a card, a problem, a return or restart, a result or the run's end.
fn item_row(ctx: &mut Ctx, item: &Item, timeline: &Timeline) -> Element {
  match &item.kind {
    ItemKind::AgentMessage { agent, step, text } => message::agent_message(ctx, (agent, step), item.at, text),
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
    ItemKind::Mark(mark) => mark_row(ctx, mark),
    ItemKind::Result(card) => cards::result(ctx, card, timeline),
    ItemKind::End(end) => cards::end_marker(ctx, end, (timeline.run_number, item.at)),
    ItemKind::Step(_) | ItemKind::Tools(_) => unreachable_row(),
  }
}

/// Steps and tool groups have rows of their own ([`Row::Step`], [`Row::Group`]).
fn unreachable_row() -> Element {
  Column::new().into()
}

/// A row's room above and below it.
fn spaced(content: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Sm)
    .child(content)
    .into()
}
