//! A module's burndown on a canvas, drawn with the Usage page's chart pieces (`screens::usage::plot`): the grid and
//! its labels, the days on the x axis, the remaining work as a line in the first series color, the ideal line dashed
//! in `text_muted`, and under them each day's scope changes, work added in the third series color and work removed
//! in the second, as thin bars from the baseline. A day still to come has no remaining work, so the line stops at
//! today; from there the delivery forecast's lines (ORC-50) fall to nothing at its 50% date, finely dashed in the first
//! series color, and at its 85% date in the fourth.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  canvas::{CanvasHandle, CanvasItem},
  components::Canvas,
  core::ElementRef,
  node::{Element, dimension::Dimension},
};
use parking_lot::Mutex;

use crate::{
  screens::usage::plot::{self, AxisName, AxisText, Frame, LABEL_GAP, LABEL_LINE, Redraw, TOP_ROOM, XLabel},
  theme::usage::ChartStyle,
};

/// The burndown's plot, taller than the Usage charts' (`Plot` `a4EQ1a`).
const PLOT_HEIGHT: f32 = 240.0;
/// Its canvas: the room over the plot, the plot and the x axis, 266 as `Plot` with `X axis` `EQ9bc`.
const CHART_HEIGHT: f32 = TOP_ROOM + PLOT_HEIGHT + LABEL_GAP + LABEL_LINE;
const LINE_WIDTH: f32 = 1.5;
const POINT_SIZE: f32 = 6.0;
/// The ideal line's dash (as the warning threshold of *Plan limits* is dashed, `bsp8a`).
const DASH: [f32; 2] = [4.0, 3.0];
/// The forecast lines' finer dash, apart from the ideal line's.
const FORECAST_DASH: [f32; 2] = [2.0, 3.0];
/// The series color of the 85% line (Other accounts' on the Usage page).
const SAFE_SERIES: usize = 3;
/// How wide a day's scope change bars are, at most, out of its slot.
const BAR_SHARE: f32 = 0.25;

/// One day of the chart.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartDay {
  pub label: Option<AxisName>,
  pub remaining: Option<u32>,
  pub ideal: Option<f64>,
  /// The forecast's 50% and 85% lines that day.
  pub likely: Option<f64>,
  pub safe: Option<f64>,
  pub added: u32,
  pub removed: u32,
  /// How the day and its remaining work read, for its canvas item.
  pub name: Arc<str>,
  pub value: Arc<str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChartProps {
  pub days: Arc<[ChartDay]>,
}

impl DevtoolsInspectable for ChartProps {}

#[derive(Clone)]
struct Plot {
  props: ChartProps,
  style: ChartStyle,
}

pub struct BurndownChart {
  reference: ElementRef,
  plot: Arc<Mutex<Option<Plot>>>,
  redraw: Redraw,
}

impl Component for BurndownChart {
  type Props = ChartProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      reference: ctx.element_ref(),
      plot: Arc::new(Mutex::new(None)),
      redraw: Redraw::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let plot = Plot {
      props: ctx.props::<ChartProps>().clone(),
      style: ChartStyle::of(ctx),
    };
    if let Some(canvas) = self.reference.as_canvas().filter(CanvasHandle::is_attached) {
      draw(&canvas, &plot);
    }
    *self.plot.lock() = Some(plot);
    Canvas::new()
      .ref_element(self.reference.clone())
      .id("module-burndown-chart")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(CHART_HEIGHT))
  }

  fn after_layout(&self) {
    if let Some(canvas) = self.reference.as_canvas() {
      self.redraw.bind(&canvas, &self.plot, draw);
    }
  }
}

/// The largest figure the chart shows, which its scale reaches.
fn largest(days: &[ChartDay]) -> u64 {
  days
    .iter()
    .map(|day| {
      let ideal = day.ideal.map_or(0, |ideal| ideal.ceil() as u64);
      u64::from(day.remaining.unwrap_or(0).max(day.added).max(day.removed)).max(ideal)
    })
    .max()
    .unwrap_or(0)
}

fn draw(canvas: &CanvasHandle, plot: &Plot) {
  let context = canvas.context_2d();
  context.reset();
  let frame = Frame::of(canvas, 0.0);
  let days = &plot.props.days;
  let (top, step) = plot::scale(largest(days));
  plot::grid(&context, frame, &plot.style, (top, step), |value| value.to_string());
  let center = |index: usize| {
    let (slot, width) = frame.slot(index, days.len());
    slot + width / 2.0
  };
  let mut items = Vec::new();
  for (index, day) in days.iter().enumerate() {
    if let Some(name) = &day.label {
      let label = XLabel {
        name,
        kind: AxisText::Day,
        x: center(index),
      };
      items.extend(plot::x_label(&context, frame, &plot.style, &label, index));
    }
    draw_changes(&context, (frame, top as f64), plot, (index, day));
  }
  context.set_line_width(LINE_WIDTH);
  context.set_stroke_style(plot.style.single);
  context.set_line_dash(&DASH);
  stroke(
    &context,
    days
      .iter()
      .enumerate()
      .map(|(index, day)| day.ideal.map(|ideal| (center(index), frame.y(ideal, top as f64)))),
  );
  draw_forecast(&context, (frame, top as f64), plot, &center);
  context.set_line_dash(&[]);
  context.set_stroke_style(plot.style.series[0]);
  let remaining = days.iter().enumerate().map(|(index, day)| {
    day
      .remaining
      .map(|left| (center(index), frame.y(f64::from(left), top as f64)))
  });
  stroke(&context, remaining);
  for (index, day) in days.iter().enumerate() {
    if let Some(left) = day.remaining {
      let (x, y) = (center(index), frame.y(f64::from(left), top as f64));
      items.push(
        CanvasItem::point(format!("remaining-{index}"), "remaining", x, y, POINT_SIZE)
          .label(day.name.as_ref())
          .value(day.value.as_ref()),
      );
    }
  }
  canvas.set_items(items);
}

/// One of a day's forecast lines.
type Line = fn(&ChartDay) -> Option<f64>;

/// The forecast's 50% and 85% lines, where the days have them.
fn draw_forecast(
  context: &lurq::canvas::Context2D,
  (frame, top): (Frame, f64),
  plot: &Plot,
  center: &dyn Fn(usize) -> f32,
) {
  let days = &plot.props.days;
  context.set_line_dash(&FORECAST_DASH);
  let lines: [(Line, usize); 2] = [(|day| day.safe, SAFE_SERIES), (|day| day.likely, 0)];
  for (value, series) in lines {
    context.set_stroke_style(plot.style.series[series]);
    stroke(
      context,
      days
        .iter()
        .enumerate()
        .map(|(index, day)| value(day).map(|left| (center(index), frame.y(left, top)))),
    );
  }
}

/// A day's added and removed work, side by side in its slot from the baseline.
fn draw_changes(
  context: &lurq::canvas::Context2D,
  (frame, top): (Frame, f64),
  plot: &Plot,
  (index, day): (usize, &ChartDay),
) {
  let (slot, width) = frame.slot(index, plot.props.days.len());
  let bar = (width * BAR_SHARE).max(1.0);
  let middle = slot + width / 2.0;
  for (amount, color, left) in [
    (day.added, plot.style.series[2], middle - bar),
    (day.removed, plot.style.series[1], middle),
  ] {
    if amount == 0 {
      continue;
    }
    let y = frame.y(f64::from(amount), top);
    context.set_fill_style(color);
    context.fill_rect(left, y, bar, frame.bottom - y);
  }
}

/// A line through the points, broken where a day has none.
fn stroke(context: &lurq::canvas::Context2D, points: impl Iterator<Item = Option<(f32, f32)>>) {
  let mut previous: Option<(f32, f32)> = None;
  for point in points {
    if let (Some((from_x, from_y)), Some((x, y))) = (previous, point) {
      context.begin_path();
      context.move_to(from_x, from_y);
      context.line_to(x, y);
      context.stroke();
    }
    previous = point;
  }
}
