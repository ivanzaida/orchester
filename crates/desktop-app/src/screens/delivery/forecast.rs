//! *Delivery forecast*, a Usage card (`screens::usage::card`): for the project's queued tasks and each open module, the
//! tasks left, the days by which half and 85 in a hundred of the Monte Carlo's trials were done (or *Not enough history
//! yet*), and how much history that rests on; a module's row opens its page, whose burndown draws the forecast too.
//! The rule the forecast follows is written under the table.

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::id::ProjectId;
use orchester_workspace::delivery::{Delivery, Work, WorkForecast};

use super::words::{history, outlook};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route},
  screens::usage::{
    card::{Card, footnote},
    table::{Cell, Density, Width, column, opening, row, table},
    words::exact,
  },
};

const FIGURE_COLUMN: f32 = 96.0;
const DATE_COLUMN: f32 = 150.0;
const HISTORY_COLUMN: f32 = 150.0;

pub fn card(ctx: &mut Ctx, project: ProjectId, delivery: &Delivery) -> Element {
  let columns = [
    column(ctx.t("delivery.columns.work"), Width::Flex(1.0), false),
    column(ctx.t("delivery.columns.remaining"), Width::Px(FIGURE_COLUMN), true),
    column(ctx.t("delivery.columns.likely"), Width::Px(DATE_COLUMN), false),
    column(ctx.t("delivery.columns.safe"), Width::Px(DATE_COLUMN), false),
    column(ctx.t("delivery.columns.basis"), Width::Px(HISTORY_COLUMN), false),
  ];
  let navigation = Navigation::of(ctx);
  let rows = delivery
    .forecasts
    .iter()
    .enumerate()
    .map(|(index, forecast)| {
      let line = row(
        &format!("delivery-forecast-row-{index}"),
        &columns,
        Density::Data,
        cells(ctx, forecast),
      );
      match &forecast.work {
        Work::Queued => line.into(),
        Work::Module { id, .. } => opening(line, navigation.to(Route::Project(project, ProjectPage::Module(*id)))),
      }
    })
    .collect();
  Card::new("delivery-forecast", ctx.t("delivery.forecast.title"))
    .subtitle(ctx.t("delivery.forecast.subtitle"))
    .close()
    .body([
      table(ctx, "delivery-forecast-table", &columns, Density::Data, rows),
      footnote("delivery-forecast-rule", &ctx.t("delivery.forecast.rule")),
    ])
}

fn cells(ctx: &Ctx, forecast: &WorkForecast) -> Vec<Cell> {
  let work = match &forecast.work {
    Work::Queued => ctx.t("delivery.forecast.queued"),
    Work::Module { name, .. } => name.clone(),
  };
  let (likely, safe) = outlook(ctx, &forecast.forecast);
  vec![
    Cell::Text(work),
    Cell::Text(exact(ctx, u64::from(forecast.forecast.remaining))),
    Cell::Text(likely),
    Cell::Text(safe),
    Cell::Muted(history(ctx, &forecast.forecast)),
  ]
}
