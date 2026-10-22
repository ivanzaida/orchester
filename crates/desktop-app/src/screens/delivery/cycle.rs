//! *Cycle time*, a Usage card (`screens::usage::card`) with its *Chart* / *Table* switch: each task merged in the
//! period, by when, as a bar of its cycle time in hours with the median dashed across ([`super::bars`]), the median and
//! spread of its cycle and lead time under it; or the tasks as a table with their cycle and lead time and attempts.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::Signal, node::Element};
use orchester_core::usage::local_date;
use orchester_workspace::delivery::{CycleTimes, Spread};

use super::{
  Views,
  bars::{Bar, BarChart, BarsProps},
  words::{duration, hours},
};
use crate::{
  screens::usage::{
    card::{Card, footnote, state, state_room, view_switch},
    plot::{AxisName, label_every},
    table::{Cell, Density, Width, column, row, table},
    words::{day, exact, long_day},
  },
  ui::icon::Icon,
};

const LABELS: f32 = 8.0;
const KEY_COLUMN: f32 = 84.0;
const FIGURE_COLUMN: f32 = 80.0;
const HOUR_MILLIS: f64 = 3_600_000.0;

pub fn card(ctx: &mut Ctx, cycle: &CycleTimes, views: &Signal<Views>) -> Element {
  let tabled = views.get().cycle;
  let switch = {
    let views = views.clone();
    view_switch(ctx, "delivery-cycle-view", tabled, false, move |table| {
      views.update(|views| views.cycle = table);
    })
  };
  let card = Card::new("delivery-cycle", ctx.t("delivery.cycle.title"))
    .note(ctx.t("delivery.cycle.unit"))
    .subtitle(ctx.t("delivery.cycle.subtitle"))
    .trailing(switch)
    .fill();
  if cycle.rows.is_empty() {
    let empty = state(
      "delivery-cycle-empty",
      (
        Icon::ChartColumn,
        &ctx.t("delivery.cycle.empty.title"),
        &ctx.t("delivery.cycle.empty.detail"),
      ),
      None,
    );
    return card.body([state_room(empty)]);
  }
  let body = if tabled {
    rows_table(ctx, cycle)
  } else {
    chart(ctx, cycle)
  };
  card.body([body].into_iter().chain(summary(ctx, cycle)))
}

/// The cycle time's median and spread, then the lead time's.
fn summary(ctx: &Ctx, cycle: &CycleTimes) -> Vec<Element> {
  let spread = |key: &str, spread: Spread| {
    ctx.t_args(
      key,
      [
        ("median", &*duration(ctx, spread.median)),
        ("lower", &*duration(ctx, spread.lower)),
        ("upper", &*duration(ctx, spread.upper)),
        ("p85", &*duration(ctx, spread.p85)),
      ],
    )
  };
  let cycle_line = cycle.cycle.map_or_else(
    || ctx.t("delivery.cycle.noCycle"),
    |cycle| spread("delivery.cycle.summary", cycle),
  );
  let mut lines = vec![footnote("delivery-cycle-summary", &cycle_line)];
  if let Some(lead) = cycle.lead {
    lines.push(footnote("delivery-lead-summary", &spread("delivery.cycle.lead", lead)));
  }
  lines
}

fn chart(ctx: &mut Ctx, cycle: &CycleTimes) -> Element {
  let timed: Vec<_> = cycle.rows.iter().filter(|row| row.cycle_ms.is_some()).collect();
  let every = label_every(timed.len(), LABELS, 1.0);
  let last = timed.len().saturating_sub(1);
  let bars: Arc<[Bar]> = timed
    .iter()
    .enumerate()
    .map(|(index, row)| {
      let millis = row.cycle_ms.unwrap_or_default();
      Bar {
        label: (last - index)
          .is_multiple_of(every)
          .then(|| AxisName::plain(day(ctx, local_date(row.merged_at)))),
        value: millis as f64 / HOUR_MILLIS,
        name: ctx.t_args("delivery.cycle.taskName", [("key", &*row.key), ("title", &*row.title)]),
        text: hours(ctx, millis),
      }
    })
    .collect();
  ctx.mount_keyed::<BarChart>(
    "delivery-cycle-bars",
    BarsProps {
      id: "delivery-cycle-chart",
      bars,
      reference: cycle.cycle.map(|spread| spread.median as f64 / HOUR_MILLIS),
    },
  )
}

fn rows_table(ctx: &Ctx, cycle: &CycleTimes) -> Element {
  let columns = [
    column(ctx.t("delivery.columns.task"), Width::Px(KEY_COLUMN), false),
    column(ctx.t("delivery.columns.merged"), Width::Flex(1.0), false),
    column(ctx.t("delivery.columns.cycle"), Width::Px(FIGURE_COLUMN), true),
    column(ctx.t("delivery.columns.lead"), Width::Px(FIGURE_COLUMN), true),
    column(ctx.t("delivery.columns.attempts"), Width::Px(FIGURE_COLUMN), true),
  ];
  let rows = cycle
    .rows
    .iter()
    .enumerate()
    .map(|(index, entry)| {
      let cells = vec![
        Cell::Muted(entry.key.clone()),
        Cell::Text(long_day(ctx, local_date(entry.merged_at))),
        Cell::Text(
          entry
            .cycle_ms
            .map_or_else(|| ctx.t("delivery.cycle.outside"), |millis| duration(ctx, millis)),
        ),
        Cell::Text(duration(ctx, entry.lead_ms)),
        Cell::Text(exact(ctx, u64::from(entry.attempts))),
      ];
      row(&format!("delivery-cycle-row-{index}"), &columns, Density::Data, cells).into()
    })
    .collect();
  table(ctx, "delivery-cycle-table", &columns, Density::Data, rows)
}
