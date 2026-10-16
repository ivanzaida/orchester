//! *Tokens over time* as a table with exact numbers (`Table` `fxX9i`, more than three series in `FdC7Y`): DAY,
//! a column per series with usage in the range (84 wide, its heading the CLI over the account with its swatch),
//! the measure's total and the runs; by hour only the hours that have usage; then *Total*.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::usage::{Grouping, Measure, SeriesBar};
use orchester_workspace::{display, usage::Dashboard};

use super::{
  series::series_heading,
  table::{Cell, Density, TableColumn, Width, column, row, table, total_row},
  words,
};

/// A series' column, the measure's total and the runs: beside *Plan limits* (`fxX9i`, `d3lc3`), and taking
/// the whole row with more than three series (`FdC7Y`).
pub const NARROW: (f32, f32, f32) = (84.0, 84.0, 36.0);
const WIDE: (f32, f32, f32) = (120.0, 110.0, 50.0);

/// The bars as a table.
pub fn tokens_table(ctx: &Ctx, dashboard: &Dashboard, measure: Measure) -> Element {
  let series = dashboard.shown_series();
  let (series_width, figure_width, runs_width) = if needs_row(dashboard) { WIDE } else { NARROW };
  let hourly = dashboard.grouping == Grouping::Hour;
  let mut columns: Vec<TableColumn> = vec![column(
    ctx.t(if hourly { "usage.table.hour" } else { "usage.table.day" }),
    Width::Flex(1.0),
    false,
  )];
  columns.extend(series.iter().map(|shown| {
    let (over, heading) = series_heading(ctx, &shown.label);
    column(heading, Width::Px(series_width), true)
      .over(over)
      .swatch(shown.series)
  }));
  columns.push(column(ctx.t(heading_key(measure)), Width::Px(figure_width), true));
  columns.push(column(ctx.t("usage.table.runs"), Width::Px(runs_width), true));
  let bars = &dashboard.report.series;
  let figures = |bar: &SeriesBar| -> Vec<u64> {
    let split = dashboard.split(&bar.by_account);
    series.iter().map(|shown| measure.of(split.of(shown.series))).collect()
  };
  let mut sums = vec![0_u64; series.len()];
  let mut rows: Vec<Element> = Vec::new();
  for (index, bar) in bars.iter().enumerate() {
    let values = figures(bar);
    for (sum, value) in sums.iter_mut().zip(&values) {
      *sum += value;
    }
    if hourly && bar.total() == Default::default() {
      continue;
    }
    let mut cells = vec![Cell::Text(row_name(ctx, dashboard, bar))];
    cells.extend(values.iter().map(|value| Cell::Text(words::exact(ctx, *value))));
    cells.push(Cell::Text(words::exact(ctx, measure.of(bar.total()))));
    cells.push(Cell::Text(bar.runs.to_string().into()));
    rows.push(
      row(
        &format!("usage-tokens-table-row-{index}"),
        &columns,
        Density::Data,
        cells,
      )
      .into(),
    );
  }
  let mut total = vec![Cell::Strong(ctx.t("usage.table.total"))];
  total.extend(sums.iter().map(|sum| Cell::Strong(words::exact(ctx, *sum))));
  total.push(Cell::Strong(words::exact(
    ctx,
    bars.iter().map(|bar| measure.of(bar.total())).sum(),
  )));
  // A run can report in several hours or days, so the total is the period's, not the rows' sum.
  total.push(Cell::Strong(dashboard.report.summary.runs.to_string().into()));
  rows.push(total_row("usage-tokens-table-total", &columns, total).into());
  table(ctx, "usage-tokens-table", &columns, Density::Data, rows)
}

/// What the table's figures are, and how far today counts when the range has it.
pub fn table_footnote(ctx: &Ctx, dashboard: &Dashboard) -> Arc<str> {
  if dashboard.has_today() {
    ctx.t_args(
      "usage.tokens.tableFootnoteToday",
      [("time", display::clock(dashboard.now).as_str())],
    )
  } else {
    ctx.t("usage.tokens.tableFootnote")
  }
}

/// Whether the table needs the whole row: more than three series columns do not fit beside *Plan limits*
/// (`FdC7Y`).
pub fn needs_row(dashboard: &Dashboard) -> bool {
  dashboard.shown_series().len() > orchester_core::usage::OWN_SERIES
}

/// A row's name in the table: `Wed, Sep 24`, *Today* (the footnote says until when), or an hour.
fn row_name(ctx: &Ctx, dashboard: &Dashboard, bar: &SeriesBar) -> Arc<str> {
  if let Some(hour) = bar.bucket.hour {
    return ctx.t_args(
      "usage.date.dateTime",
      [
        ("day", &*words::weekday(ctx, bar.bucket.date)),
        ("time", words::hour(hour).as_str()),
      ],
    );
  }
  if bar.bucket.date == dashboard.today() {
    return ctx.t("usage.date.today");
  }
  words::long_day(ctx, bar.bucket.date)
}

const fn heading_key(measure: Measure) -> &'static str {
  match measure {
    Measure::Spent => "usage.table.measure.spent",
    Measure::Cached => "usage.table.measure.cached",
    Measure::Output => "usage.table.measure.output",
  }
}
