//! *Where time goes*, a Usage card (`screens::usage::card`) with its *Chart* / *Table* switch: active agent hours per
//! pipeline step of the executions that started in the period, most first, as bars ([`super::bars`]), or a table with
//! each step's executions, how many of them a later attempt returned and the return rate; and under either the
//! attempts per merged task.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::Signal, node::Element};
use orchester_workspace::delivery::{StepRow, WhereTimeGoes};

use super::{
  Views,
  bars::{Bar, BarChart, BarsProps},
  words::hours,
};
use crate::{
  screens::usage::{
    card::{Card, footnote, state, state_room, view_switch},
    plot::AxisName,
    table::{Cell, Density, Width, column, row, table},
    words::{duration, exact},
  },
  ui::icon::Icon,
};

const FIGURE_COLUMN: f32 = 96.0;
const HOUR_MILLIS: f64 = 3_600_000.0;

pub fn card(ctx: &mut Ctx, time: &WhereTimeGoes, views: &Signal<Views>) -> Element {
  let tabled = views.get().time;
  let switch = {
    let views = views.clone();
    view_switch(ctx, "delivery-time-view", tabled, false, move |table| {
      views.update(|views| views.time = table);
    })
  };
  let card = Card::new("delivery-time", ctx.t("delivery.time.title"))
    .note(ctx.t("delivery.cycle.unit"))
    .subtitle(ctx.t("delivery.time.subtitle"))
    .trailing(switch)
    .fill();
  if time.steps.is_empty() {
    let empty = state(
      "delivery-time-empty",
      (
        Icon::ChartColumn,
        &ctx.t("delivery.time.empty.title"),
        &ctx.t("delivery.time.empty.detail"),
      ),
      None,
    );
    return card.body([state_room(empty)]);
  }
  let body = if tabled {
    steps_table(ctx, &time.steps)
  } else {
    chart(ctx, &time.steps)
  };
  let attempts = time.attempts_per_task().map_or_else(
    || ctx.t("delivery.time.noAttempts"),
    |value| ctx.t_args("delivery.time.attempts", [("value", format!("{value:.1}").as_str())]),
  );
  card.body([body, footnote("delivery-time-attempts", &attempts)])
}

fn rate(ctx: &Ctx, step: &StepRow) -> Arc<str> {
  ctx.t_args(
    "delivery.time.percent",
    [("value", step.return_percent().to_string().as_str())],
  )
}

fn chart(ctx: &mut Ctx, steps: &[StepRow]) -> Element {
  let bars: Arc<[Bar]> = steps
    .iter()
    .map(|step| Bar {
      label: Some(AxisName::plain(step.step.clone())),
      value: step.active_ms as f64 / HOUR_MILLIS,
      name: step.step.clone(),
      text: ctx.t_args(
        "delivery.time.point",
        [("hours", &*hours(ctx, step.active_ms)), ("rate", &*rate(ctx, step))],
      ),
    })
    .collect();
  ctx.mount_keyed::<BarChart>(
    "delivery-time-bars",
    BarsProps {
      id: "delivery-time-chart",
      bars,
      reference: None,
    },
  )
}

fn steps_table(ctx: &Ctx, steps: &[StepRow]) -> Element {
  let columns = [
    column(ctx.t("delivery.columns.step"), Width::Flex(1.0), false),
    column(ctx.t("delivery.columns.active"), Width::Px(FIGURE_COLUMN), true),
    column(ctx.t("delivery.columns.executions"), Width::Px(FIGURE_COLUMN), true),
    column(ctx.t("delivery.columns.returned"), Width::Px(FIGURE_COLUMN), true),
    column(ctx.t("delivery.columns.rate"), Width::Px(FIGURE_COLUMN), true),
  ];
  let rows = steps
    .iter()
    .enumerate()
    .map(|(index, step)| {
      let cells = vec![
        Cell::Text(step.step.clone()),
        Cell::Text(duration(ctx, step.active_ms)),
        Cell::Text(exact(ctx, u64::from(step.executions))),
        Cell::Text(exact(ctx, u64::from(step.returned))),
        Cell::Muted(rate(ctx, step)),
      ];
      row(&format!("delivery-time-row-{index}"), &columns, Density::Data, cells).into()
    })
    .collect();
  table(ctx, "delivery-time-table", &columns, Density::Data, rows)
}
