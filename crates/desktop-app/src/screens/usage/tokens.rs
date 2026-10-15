//! *Tokens over time* (`Card · Tokens over time` `w1aVOX`, `EA7QF`): stacked bars by account per hour or day
//! with *Spent* / *Cached* / *Output*, a legend with each series' total ([`super::tokens_legend`]), a tooltip
//! on the hovered bar (`Chart tooltip` `G7cuH`) and the same data as a table ([`super::tokens_table`]).

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::MouseEvent,
  },
  canvas::CanvasHandle,
  components::{Canvas, Column, Stack},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::usage::{AccountSeries, Grouping, Measure, SeriesBar};
use orchester_workspace::{display, usage::Dashboard};
use parking_lot::Mutex;

use super::{
  Views,
  card::{Card, footnote, push_down, view_switch},
  plot::{AxisName, AxisText, CHART_HEIGHT, Frame, Redraw, label_every},
  series::{account_text, series_swatch, series_text},
  tokens_draw::{TokenPlot, draw},
  tokens_legend::legend,
  tokens_table::tokens_table,
  tooltip, words,
};
use crate::{
  theme::{UsageSpacing, usage::ChartStyle},
  ui::{
    pressable::Focus,
    tabs::{Segment, SegmentFace, segmented},
  },
};

/// About how wide an x axis label is, to leave out labels that would crowd.
const LABEL_ROOM: f32 = 56.0;

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
    chart_block(legend(ctx, dashboard, shown.measure), chart),
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
        .map(|bar| dashboard.split(&bar.by_account).series.map(|usage| measure.of(usage)))
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
          (self.reference.clone(), CHART_HEIGHT),
          (tokens_tip(ctx, &dashboard, bar, measure), tooltip::WIDTH),
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

/// The hovered bar's tooltip (`EA7QF`): its day, each series with usage in the range with its exact count,
/// top to bottom in series order, Other accounts opened to its accounts, then the measure's total and the runs.
fn tokens_tip(ctx: &Ctx, dashboard: &Dashboard, bar: &SeriesBar, measure: Measure) -> Element {
  let title = match bar.bucket.hour {
    Some(_) => bar_name(ctx, bar),
    None => words::long_day(ctx, bar.bucket.date),
  };
  let split = dashboard.split(&bar.by_account);
  let mut rows: Vec<Element> = Vec::new();
  for series in dashboard.shown_series() {
    rows.push(tooltip::line(
      Some(series_swatch(series)),
      &series_text(ctx, &dashboard.series_label(series)),
      Some(&words::exact(ctx, measure.of(split.of(series)))),
    ));
    if series == AccountSeries::Other && split.others.len() > 1 {
      rows.extend(split.others.iter().map(|(account, usage)| {
        tooltip::detail_line(
          &account_text(ctx, &dashboard.account_label(*account)),
          &words::exact(ctx, measure.of(*usage)),
        )
      }));
    }
  }
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
