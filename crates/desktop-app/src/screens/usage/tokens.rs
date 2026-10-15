//! *Tokens over time* (`Card · Tokens over time` `w1aVOX`): stacked bars by CLI per hour or day with
//! *Spent* / *Cached* / *Output*, a legend with each CLI's total, a tooltip on the hovered bar (`Chart
//! tooltip` `G7cuH`: the day, each CLI's exact count with its swatch, spent and runs) and the same data as
//! a table with exact numbers (`Table` `fxX9i`).

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::MouseEvent,
    theme::{SpacingSize, TypographyStyle},
  },
  canvas::CanvasHandle,
  components::{Canvas, Column, Row, Spacer, Stack, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::usage::{CliSeries, Grouping, Measure, SeriesBar};
use orchester_workspace::{display, usage::Dashboard};
use parking_lot::Mutex;

use super::{
  Views,
  card::{Card, footnote, legend_item, push_down, view_switch},
  plot::{AxisName, AxisText, CHART_HEIGHT, Frame, Redraw, label_every},
  table::{Cell, Density, TableColumn, Width, column, row, table},
  tokens_draw::{TokenPlot, draw},
  tooltip, words,
};
use crate::{
  theme::{UsageSpacing, usage::ChartStyle},
  ui::{
    pressable::Focus,
    tabs::{Segment, SegmentFace, segmented},
  },
};

/// The legend's room over the chart, as tall as *Plan limits*' two rows of windows beside it, so the two
/// plots line up (`Legend` `CuobA`).
pub const LEGEND_HEIGHT: f32 = 72.0;
/// About how wide an x axis label is, to leave out labels that would crowd.
const LABEL_ROOM: f32 = 56.0;
/// Table columns (`fxX9i`): a CLI's, spent, runs.
const FIGURE_COLUMN: f32 = 96.0;
const RUNS_COLUMN: f32 = 44.0;

/// The card, its body as `views` say.
pub fn card(ctx: &mut Ctx, dashboard: &Arc<Dashboard>, views: &Signal<Views>) -> Element {
  let shown = views.get();
  let card = Card::new("usage-tokens", ctx.t("usage.tokens.title"))
    .trailing(measure_switch(ctx, views))
    .trailing(tokens_view_switch(ctx, views))
    .fill();
  if shown.tokens_table {
    return card.body([
      tokens_table(ctx, dashboard, shown.measure),
      push_down(),
      footnote("usage-tokens-footnote", &ctx.t("usage.tokens.tableFootnote")),
    ]);
  }
  let chart = ctx.mount_keyed::<TokensChart>(
    "usage-tokens-chart",
    ChartProps {
      dashboard: Arc::clone(dashboard),
      measure: shown.measure,
    },
  );
  card.body([
    chart_block(totals(ctx, dashboard, shown.measure), chart),
    footnote("usage-tokens-footnote", &chart_footnote(ctx, dashboard)),
  ])
}

/// A legend over its chart; the chart's room for its top label overlaps the card's gap above it.
pub fn chart_block(legend: Element, chart: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(UsageSpacing::AboveChart.role())
    .child(legend)
    .child(chart)
    .into()
}

/// *Spent* / *Cached* / *Output* (`Spent / Cached / Output` `lIQvg`).
pub fn measure_switch(ctx: &Ctx, views: &Signal<Views>) -> Element {
  let chosen = views.get().measure;
  let segment = |id: &str, key: &str, measure: Measure| {
    let views = views.clone();
    Segment::new(
      format!("usage-measure-{id}"),
      SegmentFace::Text(ctx.t(key)),
      chosen == measure,
      move || views.update(|views| views.measure = measure),
    )
  };
  segmented(
    "usage-measure",
    [
      segment("spent", "usage.measure.spent", Measure::Spent),
      segment("cached", "usage.measure.cached", Measure::Cached),
      segment("output", "usage.measure.output", Measure::Output),
    ],
    Focus::Pointer,
  )
  .into()
}

pub fn tokens_view_switch(ctx: &Ctx, views: &Signal<Views>) -> Element {
  let views = views.clone();
  view_switch(
    ctx,
    "usage-tokens-view",
    views.get().tokens_table,
    false,
    move |table| {
      views.update(|views| views.tokens_table = table);
    },
  )
}

#[derive(Clone)]
struct ChartProps {
  dashboard: Arc<Dashboard>,
  measure: Measure,
}

impl PartialEq for ChartProps {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.dashboard, &other.dashboard) && self.measure == other.measure
  }
}

impl DevtoolsInspectable for ChartProps {}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Hovered(Option<usize>);

impl DevtoolsInspectable for Hovered {}

struct TokensChart {
  reference: ElementRef,
  hovered: Signal<Hovered>,
  /// What the canvas shows, for the redraws its size changes ask for.
  plot: Arc<Mutex<Option<TokenPlot>>>,
  redraw: Redraw,
}

impl Component for TokensChart {
  type Props = ChartProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      reference: ctx.element_ref(),
      hovered: ctx.signal(Hovered::default()),
      plot: Arc::new(Mutex::new(None)),
      redraw: Redraw::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let ChartProps { dashboard, measure } = ctx.props::<ChartProps>().clone();
    let bars = &dashboard.report.series;
    let hovered = self.hovered.get().0.filter(|index| *index < bars.len());
    let width = self.reference.as_canvas().map(|canvas| canvas.size().width);
    let plot = TokenPlot {
      bars: bars
        .iter()
        .map(|bar| bar.by_cli.map(|usage| measure.of(usage)))
        .collect(),
      labels: labels(ctx, &dashboard, width),
      label_kind: if dashboard.grouping == Grouping::Hour && dashboard.range.days() == 1 {
        AxisText::Clock
      } else {
        AxisText::Day
      },
      items: bars
        .iter()
        .map(|bar| (bar_name(ctx, bar), words::exact(ctx, measure.of(bar.total()))))
        .collect(),
      hovered,
      style: ChartStyle::of(ctx),
    };
    if let Some(canvas) = self.reference.as_canvas().filter(CanvasHandle::is_attached) {
      draw(&canvas, &plot);
    }
    *self.plot.lock() = Some(plot);
    let count = bars.len();
    let (reference, signal, shared) = (self.reference.clone(), self.hovered.clone(), Arc::clone(&self.plot));
    let point = move |event: MouseEvent| {
      let Some(canvas) = reference.as_canvas() else {
        return;
      };
      let index = canvas
        .point_from_window(event.x, event.y)
        .and_then(|(x, _)| Frame::of(&canvas, 0.0).slot_at(x, count));
      hover(&canvas, &shared, &signal, index);
    };
    let (reference, signal, shared) = (self.reference.clone(), self.hovered.clone(), Arc::clone(&self.plot));
    let leave = move || {
      if let Some(canvas) = reference.as_canvas() {
        hover(&canvas, &shared, &signal, None);
      }
    };
    let canvas = Canvas::new()
      .ref_element(self.reference.clone())
      .id("usage-tokens-chart")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(CHART_HEIGHT))
      .on_mouse_move(point)
      .on_mouse_leave(leave);
    let tip = hovered
      .and_then(|index| Some((index, bars.get(index)?)))
      .map(|(index, bar)| {
        let width = width.unwrap_or(0.0);
        let frame = Frame {
          left: super::plot::AXIS_GUTTER,
          top: 0.0,
          right: width,
          bottom: 0.0,
          edge: width,
        };
        let (slot, slot_width) = frame.slot(index, count);
        tooltip::beside(
          tokens_tip(ctx, &dashboard, bar, measure),
          (slot, slot + slot_width),
          width,
        )
      });
    Stack::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(CHART_HEIGHT))
      .child(canvas)
      .with_children(tip)
  }

  fn after_layout(&self) {
    if let Some(canvas) = self.reference.as_canvas() {
      self.redraw.bind(&canvas, &self.plot, draw);
    }
  }
}

/// Marks bar `index` hovered, or none, and redraws at once; the tooltip follows the signal.
fn hover(canvas: &CanvasHandle, shared: &Mutex<Option<TokenPlot>>, signal: &Signal<Hovered>, index: Option<usize>) {
  if signal.get_untracked().0 == index {
    return;
  }
  if let Some(plot) = shared.lock().as_mut() {
    plot.hovered = index;
    draw(canvas, plot);
  }
  signal.set(Hovered(index));
}

/// A bar's name: its day (`Mon 29`) or its hour (`Mon 29, 14:00`).
fn bar_name(ctx: &Ctx, bar: &SeriesBar) -> Arc<str> {
  let day = words::weekday(ctx, bar.bucket.date);
  match bar.bucket.hour {
    Some(hour) => ctx.t_args(
      "usage.date.dateTime",
      [("day", &*day), ("time", words::hour(hour).as_str())],
    ),
    None => day,
  }
}

/// The x axis labels: days, *Today* for today's bar, or hours of one day; every few where many, counted
/// back from the last bar so it always has its label and none crowds it.
fn labels(ctx: &Ctx, dashboard: &Dashboard, width: Option<f32>) -> Vec<Option<AxisName>> {
  let bars = &dashboard.report.series;
  let today = dashboard.today();
  let plot_width = width.map_or(f32::MAX, |width| width - super::plot::AXIS_GUTTER);
  let days = dashboard.range.days();
  let every = label_every(bars.len(), plot_width, LABEL_ROOM);
  bars
    .iter()
    .enumerate()
    .map(|(index, bar)| match (dashboard.grouping, bar.bucket.hour) {
      (Grouping::Hour, Some(hour)) if days == 1 => (hour % 3 == 0).then(|| AxisName::plain(words::hour(hour).into())),
      (Grouping::Hour, Some(hour)) => (hour == 0).then(|| AxisName::day(ctx, bar.bucket.date, today)),
      _ if !(bars.len() - 1 - index).is_multiple_of(every) => None,
      _ => Some(AxisName::day(ctx, bar.bucket.date, today)),
    })
    .collect()
}

/// The hovered bar's tooltip: its day, each CLI's exact count, spent and runs.
fn tokens_tip(ctx: &Ctx, dashboard: &Dashboard, bar: &SeriesBar, measure: Measure) -> Element {
  let title = match bar.bucket.hour {
    Some(_) => bar_name(ctx, bar),
    None => words::long_day(ctx, bar.bucket.date),
  };
  let mut rows: Vec<Element> = CliSeries::ALL
    .into_iter()
    .filter(|series| shown(dashboard, *series))
    .map(|series| {
      tooltip::line(
        Some(super::card::series_color(series)),
        &words::series_name(ctx, series),
        Some(&words::exact(ctx, measure.of(bar.by_cli[series.index()]))),
      )
    })
    .collect();
  rows.push(tooltip::line(
    None,
    &ctx.t(measure_key(measure)),
    Some(&words::exact(ctx, measure.of(bar.total()))),
  ));
  rows.push(tooltip::line(
    None,
    &ctx.t("usage.tokens.runs"),
    Some(&bar.runs.to_string()),
  ));
  tooltip::tip("usage-tokens-tooltip", &title, rows, None)
}

const fn measure_key(measure: Measure) -> &'static str {
  match measure {
    Measure::Spent => "usage.measure.spent",
    Measure::Cached => "usage.measure.cached",
    Measure::Output => "usage.measure.output",
  }
}

/// Whether a CLI's series has anything in the range.
fn shown(dashboard: &Dashboard, series: CliSeries) -> bool {
  dashboard
    .report
    .series
    .iter()
    .any(|bar| bar.by_cli[series.index()] != Default::default())
}

/// What *Spent* counts, and how far today counts when the range has it.
fn chart_footnote(ctx: &Ctx, dashboard: &Dashboard) -> Arc<str> {
  if dashboard.has_today() {
    ctx.t_args(
      "usage.tokens.footnoteToday",
      [("time", display::clock(dashboard.now).as_str())],
    )
  } else {
    ctx.t("usage.tokens.footnote")
  }
}

/// What the bars count, per day or per hour.
fn caption(ctx: &Ctx, dashboard: &Dashboard, measure: Measure) -> Arc<str> {
  ctx.t(match (measure, dashboard.grouping) {
    (Measure::Spent, Grouping::Day) => "usage.tokens.perDay.spent",
    (Measure::Cached, Grouping::Day) => "usage.tokens.perDay.cached",
    (Measure::Output, Grouping::Day) => "usage.tokens.perDay.output",
    (Measure::Spent, Grouping::Hour) => "usage.tokens.perHour.spent",
    (Measure::Cached, Grouping::Hour) => "usage.tokens.perHour.cached",
    (Measure::Output, Grouping::Hour) => "usage.tokens.perHour.output",
  })
}

/// The legend: each CLI with its total for the measure, and what the bars count at the end.
fn totals(ctx: &Ctx, dashboard: &Dashboard, measure: Measure) -> Element {
  let items = CliSeries::ALL
    .into_iter()
    .filter(|series| shown(dashboard, *series))
    .map(|series| {
      let total: u64 = dashboard
        .report
        .series
        .iter()
        .map(|bar| measure.of(bar.by_cli[series.index()]))
        .sum();
      legend_item(
        super::card::series_color(series),
        &words::series_name(ctx, series),
        Some(&words::tokens(total)),
      )
    });
  Column::new()
    .id("usage-tokens-legend")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(LEGEND_HEIGHT))
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Lg)
        .align_items(Alignment::Center)
        .with_children(items)
        .child(Spacer::new().flex(1.0))
        .child(
          Text::new(&caption(ctx, dashboard, measure))
            .id("usage-tokens-caption")
            .variant(TypographyStyle::Caption)
            .nowrap(),
        ),
    )
    .into()
}

/// A row's name in the table: `Wed, Sep 24`, `Today, until 14:20`, or an hour.
fn row_name(ctx: &Ctx, dashboard: &Dashboard, bar: &SeriesBar) -> Arc<str> {
  if bar.bucket.hour.is_some() {
    return bar_name(ctx, bar);
  }
  if bar.bucket.date == dashboard.today() {
    return ctx.t_args(
      "usage.tokens.todayUntil",
      [("time", display::clock(dashboard.now).as_str())],
    );
  }
  words::long_day(ctx, bar.bucket.date)
}

/// The bars as a table with exact numbers (`fxX9i`); by hour only the hours that have usage.
fn tokens_table(ctx: &Ctx, dashboard: &Dashboard, measure: Measure) -> Element {
  let series: Vec<CliSeries> = CliSeries::ALL
    .into_iter()
    .filter(|series| shown(dashboard, *series))
    .collect();
  let hourly = dashboard.grouping == Grouping::Hour;
  let mut columns: Vec<TableColumn> = vec![column(
    ctx.t(if hourly { "usage.table.hour" } else { "usage.table.day" }),
    Width::Flex(1.0),
    false,
  )];
  columns.extend(series.iter().map(|series| {
    column(words::series_heading(ctx, *series), Width::Px(FIGURE_COLUMN), true)
      .swatch(super::card::series_color(*series))
  }));
  columns.push(column(ctx.t(heading_key(measure)), Width::Px(FIGURE_COLUMN), true));
  columns.push(column(ctx.t("usage.table.runs"), Width::Px(RUNS_COLUMN), true));
  let bars = &dashboard.report.series;
  let mut rows: Vec<Element> = bars
    .iter()
    .enumerate()
    .filter(|(_, bar)| !hourly || bar.total() != Default::default())
    .map(|(index, bar)| {
      let mut cells = vec![Cell::Text(row_name(ctx, dashboard, bar))];
      cells.extend(
        series
          .iter()
          .map(|series| Cell::Text(words::exact(ctx, measure.of(bar.by_cli[series.index()])))),
      );
      cells.push(Cell::Text(words::exact(ctx, measure.of(bar.total()))));
      cells.push(Cell::Text(bar.runs.to_string().into()));
      row(
        &format!("usage-tokens-table-row-{index}"),
        &columns,
        Density::Data,
        cells,
      )
      .into()
    })
    .collect();
  let mut total = vec![Cell::Strong(ctx.t("usage.table.total"))];
  total.extend(series.iter().map(|series| {
    let sum = bars.iter().map(|bar| measure.of(bar.by_cli[series.index()])).sum();
    Cell::Strong(words::exact(ctx, sum))
  }));
  total.push(Cell::Strong(words::exact(
    ctx,
    bars.iter().map(|bar| measure.of(bar.total())).sum(),
  )));
  // A run can report in several hours or days, so the total is the period's, not the rows' sum.
  total.push(Cell::Strong(dashboard.report.summary.runs.to_string().into()));
  rows.push(row("usage-tokens-table-total", &columns, Density::Data, total).into());
  table("usage-tokens-table", &columns, Density::Data, rows)
}

const fn heading_key(measure: Measure) -> &'static str {
  match measure {
    Measure::Spent => "usage.table.measure.spent",
    Measure::Cached => "usage.table.measure.cached",
    Measure::Output => "usage.table.measure.output",
  }
}
