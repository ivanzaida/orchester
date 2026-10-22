//! *Cumulative flow*, a Usage card (`screens::usage::card`) with its *Chart* / *Table* switch: the legend of the
//! workflow statuses and the stacked bands ([`super::flow_chart`]), or each day's counts as a table.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::Element,
};
use orchester_workspace::delivery::{FLOW_STATUSES, FlowDay};

use super::{
  Views,
  flow_chart::{BAND_SERIES, FlowChart, FlowPoint, FlowProps},
};
use crate::{
  screens::usage::{
    card::{Card, state, state_room, swatch, view_switch},
    plot::{AxisName, label_every},
    table::{Cell, Density, Width, column, row, table},
    words::{exact, long_day, weekday},
  },
  theme::{ExtraColor, ExtraSpacing},
  ui::{icon::Icon, status::workflow_key},
};

const LABELS: f32 = 10.0;
const FIGURE_COLUMN: f32 = 84.0;
/// The series colors by index, as `theme::usage::ChartStyle::series` fills them.
const SERIES_COLORS: [ExtraColor; 5] = [
  ExtraColor::Chart1,
  ExtraColor::Chart2,
  ExtraColor::Chart3,
  ExtraColor::ChartOther,
  ExtraColor::ChartNone,
];

pub fn card(ctx: &mut Ctx, flow: &[FlowDay], views: &Signal<Views>) -> Element {
  let tabled = views.get().flow;
  let switch = {
    let views = views.clone();
    view_switch(ctx, "delivery-flow-view", tabled, false, move |table| {
      views.update(|views| views.flow = table);
    })
  };
  let card = Card::new("delivery-flow", ctx.t("delivery.flow.title"))
    .subtitle(ctx.t("delivery.flow.subtitle"))
    .trailing(switch)
    .fill();
  if flow.iter().all(|day| day.total() == 0) {
    let empty = state(
      "delivery-flow-empty",
      (
        Icon::ChartColumn,
        &ctx.t("delivery.flow.empty.title"),
        &ctx.t("delivery.flow.empty.detail"),
      ),
      None,
    );
    return card.body([state_room(empty)]);
  }
  let body = if tabled {
    days_table(ctx, flow)
  } else {
    Column::new()
      .spacing(SpacingSize::Sm)
      .child(legend(ctx))
      .child(chart(ctx, flow))
      .into()
  };
  card.body([body])
}

fn legend(ctx: &Ctx) -> Element {
  Row::new()
    .id("delivery-flow-legend")
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .with_children(
      FLOW_STATUSES
        .iter()
        .zip(BAND_SERIES)
        .enumerate()
        .map(|(index, (status, series))| {
          Row::new()
            .id(format!("delivery-flow-legend-{index}"))
            .spacing(ExtraSpacing::Snug.role())
            .align_items(Alignment::Center)
            .child(swatch(SERIES_COLORS[series].role()))
            .child(
              Text::new(&ctx.t(workflow_key(*status)))
                .variant(TypographyStyle::Caption)
                .nowrap(),
            )
        }),
    )
    .into()
}

/// A day's counts, `Done 4 · In review 1 · …`.
fn counts(ctx: &Ctx, day: &FlowDay) -> Arc<str> {
  let parts: Vec<Arc<str>> = FLOW_STATUSES
    .iter()
    .zip(day.counts)
    .map(|(status, count)| {
      ctx.t_args(
        "delivery.flow.count",
        [
          ("status", &*ctx.t(workflow_key(*status))),
          ("count", count.to_string().as_str()),
        ],
      )
    })
    .collect();
  parts.join(&ctx.t("delivery.flow.separator")).into()
}

fn chart(ctx: &mut Ctx, flow: &[FlowDay]) -> Element {
  let every = label_every(flow.len(), LABELS, 1.0);
  let last = flow.len().saturating_sub(1);
  let days: Arc<[FlowPoint]> = flow
    .iter()
    .enumerate()
    .map(|(index, day)| FlowPoint {
      label: (last - index)
        .is_multiple_of(every)
        .then(|| AxisName::plain(weekday(ctx, day.date))),
      counts: day.counts,
      name: long_day(ctx, day.date),
      value: counts(ctx, day),
    })
    .collect();
  ctx.mount_keyed::<FlowChart>("delivery-flow-bands", FlowProps { days })
}

fn days_table(ctx: &Ctx, flow: &[FlowDay]) -> Element {
  let columns: Vec<_> = std::iter::once(column(ctx.t("delivery.columns.day"), Width::Flex(1.0), false))
    .chain(
      FLOW_STATUSES
        .iter()
        .map(|status| column(ctx.t(workflow_key(*status)), Width::Px(FIGURE_COLUMN), true)),
    )
    .collect();
  let rows = flow
    .iter()
    .enumerate()
    .map(|(index, day)| {
      let cells = std::iter::once(Cell::Text(long_day(ctx, day.date)))
        .chain(day.counts.iter().map(|count| Cell::Text(exact(ctx, u64::from(*count)))))
        .collect();
      row(&format!("delivery-flow-row-{index}"), &columns, Density::Data, cells).into()
    })
    .collect();
  table(ctx, "delivery-flow-table", &columns, Density::Data, rows)
}
