//! *Throughput*, a Usage card (`screens::usage::card`) with its *Chart* / *Table* switch: the tasks merged per day,
//! or per week past a month, as bars ([`super::bars`]), or the same bars as a table with their story points; what
//! stands in its place when nothing was merged in the period.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::Signal, node::Element};
use orchester_core::usage::DayRange;
use orchester_workspace::delivery::{Bucketing, Delivery, ThroughputBar};

use super::{
  Views,
  bars::{Bar, BarChart, BarsProps},
};
use crate::{
  screens::usage::{
    card::{Card, state, state_room, view_switch},
    plot::{AxisName, label_every},
    table::{Cell, Density, Width, column, row, table, total_row},
    words::{day, exact, long_day, range, weekday},
  },
  ui::icon::Icon,
};

/// Bar labels the chart keeps, at most.
const LABELS: f32 = 10.0;
const FIGURE_COLUMN: f32 = 110.0;

pub fn card(ctx: &mut Ctx, delivery: &Delivery, views: &Signal<Views>) -> Element {
  let tabled = views.get().throughput;
  let switch = {
    let views = views.clone();
    view_switch(ctx, "delivery-throughput-view", tabled, false, move |table| {
      views.update(|views| views.throughput = table);
    })
  };
  let subtitle = ctx.t(match delivery.bucketing {
    Bucketing::Day => "delivery.throughput.byDay",
    Bucketing::Week => "delivery.throughput.byWeek",
  });
  let card = Card::new("delivery-throughput", ctx.t("delivery.throughput.title"))
    .subtitle(subtitle)
    .trailing(switch)
    .fill();
  let body = if delivery.throughput.iter().all(|bar| bar.tasks == 0) {
    state_room(state(
      "delivery-throughput-empty",
      (
        Icon::ChartColumn,
        &ctx.t("delivery.throughput.empty.title"),
        &ctx.t("delivery.throughput.empty.detail"),
      ),
      None,
    ))
  } else if tabled {
    bars_table(ctx, delivery)
  } else {
    chart(ctx, delivery)
  };
  card.body([body])
}

/// What a bar's days read as: the day, or the week's first and last.
fn bar_name(ctx: &Ctx, bar: &ThroughputBar) -> Arc<str> {
  if bar.first == bar.last {
    return long_day(ctx, bar.first);
  }
  DayRange::new(bar.first, bar.last).map_or_else(|_| day(ctx, bar.first), |days| range(ctx, days))
}

fn chart(ctx: &mut Ctx, delivery: &Delivery) -> Element {
  let every = label_every(delivery.throughput.len(), LABELS, 1.0);
  let last = delivery.throughput.len().saturating_sub(1);
  let bars: Arc<[Bar]> = delivery
    .throughput
    .iter()
    .enumerate()
    .map(|(index, bar)| {
      let label = match delivery.bucketing {
        Bucketing::Day => weekday(ctx, bar.first),
        Bucketing::Week => day(ctx, bar.first),
      };
      Bar {
        // Counted back from the last bar, so it always has its label, as the Usage charts do.
        label: (last - index).is_multiple_of(every).then(|| AxisName::plain(label)),
        value: f64::from(bar.tasks),
        name: bar_name(ctx, bar),
        text: ctx.t_args(
          "delivery.throughput.total",
          [
            ("tasks", bar.tasks.to_string().as_str()),
            ("points", bar.points.to_string().as_str()),
          ],
        ),
      }
    })
    .collect();
  ctx.mount_keyed::<BarChart>(
    "delivery-throughput-bars",
    BarsProps {
      id: "delivery-throughput-chart",
      bars,
      reference: None,
    },
  )
}

fn bars_table(ctx: &Ctx, delivery: &Delivery) -> Element {
  let first = match delivery.bucketing {
    Bucketing::Day => "delivery.columns.day",
    Bucketing::Week => "delivery.columns.week",
  };
  let columns = [
    column(ctx.t(first), Width::Flex(1.0), false),
    column(ctx.t("delivery.columns.tasks"), Width::Px(FIGURE_COLUMN), true),
    column(ctx.t("delivery.columns.points"), Width::Px(FIGURE_COLUMN), true),
    column(ctx.t("delivery.columns.unpointed"), Width::Px(FIGURE_COLUMN), true),
  ];
  let mut rows: Vec<Element> = delivery
    .throughput
    .iter()
    .enumerate()
    .map(|(index, bar)| {
      let cells = vec![
        Cell::Text(bar_name(ctx, bar)),
        Cell::Text(exact(ctx, u64::from(bar.tasks))),
        Cell::Text(exact(ctx, u64::from(bar.points))),
        Cell::Muted(exact(ctx, u64::from(bar.unpointed))),
      ];
      row(
        &format!("delivery-throughput-row-{index}"),
        &columns,
        Density::Data,
        cells,
      )
      .into()
    })
    .collect();
  let sum = |figure: fn(&ThroughputBar) -> u32| exact(ctx, delivery.throughput.iter().map(figure).map(u64::from).sum());
  let totals = vec![
    Cell::Strong(ctx.t("delivery.throughput.totalRow")),
    Cell::Strong(sum(|bar| bar.tasks)),
    Cell::Strong(sum(|bar| bar.points)),
    Cell::Strong(sum(|bar| bar.unpointed)),
  ];
  rows.push(total_row("delivery-throughput-total", &columns, totals).into());
  table(ctx, "delivery-throughput-table", &columns, Density::Data, rows)
}
