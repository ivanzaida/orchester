//! Draws *Plan limits* (`C2pTYF`, `e5PbG`): % used per window over the range, solid for weekly
//! windows and dashed for 5-hour ones, colored by CLI. Reports that kept coming are joined, a
//! stretch without reports is a dotted 1px join, a window that reset is not joined across the reset
//! and gets a `$border` rule; before a CLI's first report the plot is hatched. The warning threshold
//! is a dashed `$muted` line labelled with its value at its end. Nothing reaches the edges.

use std::{f32::consts::TAU, sync::Arc};

use lurq::canvas::{ArcDirection, CanvasHandle, CanvasItem, Context2D, TextAlign, TextBaseline};
use orchester_core::usage::limits::Join;

use super::plot::{self, AxisName, AxisText, Frame, XLabel};
use crate::theme::chart::{ChartStyle, DASH, DOT, GAP_LINE_WIDTH, HATCH_PITCH, LINE_WIDTH, POINT_RADIUS, RULE_WIDTH};

/// One reading on a line: its time (ms) and percent, with its name and value for its canvas item.
#[derive(Clone, Debug)]
pub struct DrawnPoint {
  pub at: i64,
  pub percent: f64,
  pub name: Arc<str>,
  pub value: Arc<str>,
}

/// One window's line.
#[derive(Clone, Debug)]
pub struct DrawnLine {
  pub series: usize,
  pub dashed: bool,
  pub points: Vec<DrawnPoint>,
  pub joins: Vec<Join>,
}

#[derive(Clone)]
pub struct LimitPlot {
  pub start: i64,
  pub end: i64,
  pub lines: Vec<DrawnLine>,
  /// When a weekly window reset, with its canvas item's name.
  pub resets: Vec<(i64, Arc<str>)>,
  /// Stretches before a CLI's first report.
  pub hatches: Vec<(i64, i64)>,
  pub threshold: u8,
  pub threshold_label: Arc<str>,
  /// The y axis labels at 0, 25, 50, 75 and 100 %.
  pub percents: [Arc<str>; 5],
  /// x axis labels at their times, and what they name: hours of one day, or days.
  pub labels: Vec<(i64, AxisName)>,
  pub label_kind: AxisText,
  /// The hovered point: its line and index.
  pub hovered: Option<(usize, usize)>,
  pub style: ChartStyle,
}

impl LimitPlot {
  pub fn x(&self, frame: Frame, at: i64) -> f32 {
    let span = (self.end - self.start).max(1) as f32;
    frame.left + (at - self.start) as f32 / span * frame.width()
  }
}

pub fn draw(canvas: &CanvasHandle, plot: &LimitPlot) {
  let context = canvas.context_2d();
  context.reset();
  let frame = Frame::of(canvas);
  hatch(&context, frame, plot);
  plot::grid(&context, frame, &plot.style, (100, 25), |value| {
    plot.percents[usize::try_from(value / 25).unwrap_or(0).min(4)].to_string()
  });
  let mut items = Vec::new();
  for (index, (at, name)) in plot.labels.iter().enumerate() {
    let label = XLabel {
      name,
      kind: plot.label_kind,
      x: plot.x(frame, *at),
    };
    items.extend(plot::x_label(&context, frame, &plot.style, &label, index));
  }
  for (index, (at, name)) in plot.resets.iter().enumerate() {
    let x = plot.x(frame, *at).round() + 0.5;
    context.set_stroke_style(plot.style.rule);
    context.set_line_width(RULE_WIDTH);
    context.set_line_dash(&[]);
    context.begin_path();
    context.move_to(x, frame.top);
    context.line_to(x, frame.bottom);
    context.stroke();
    items.push(
      CanvasItem::rect(
        format!("reset-{index}"),
        "reset",
        x - HATCH_PITCH,
        frame.top,
        HATCH_PITCH * 2.0,
        frame.height(),
      )
      .label(name.as_ref()),
    );
  }
  threshold(&context, frame, plot);
  for (line_index, line) in plot.lines.iter().enumerate() {
    draw_line(&context, frame, plot, line);
    for (point_index, point) in line.points.iter().enumerate() {
      let (x, y) = (plot.x(frame, point.at), frame.y(point.percent, 100.0));
      let last = point_index + 1 == line.points.len();
      if last || plot.hovered == Some((line_index, point_index)) {
        dot(&context, (x, y), plot.style.series[line.series]);
      }
      items.push(
        CanvasItem::point(
          format!("limit-{line_index}-{point_index}"),
          "limit",
          x,
          y,
          POINT_RADIUS * 2.0,
        )
        .label(point.name.as_ref())
        .value(point.value.as_ref()),
      );
    }
  }
  canvas.set_items(items);
}

fn draw_line(context: &Context2D, frame: Frame, plot: &LimitPlot, line: &DrawnLine) {
  context.set_stroke_style(plot.style.series[line.series]);
  for (pair, join) in line.points.windows(2).zip(&line.joins) {
    let (width, dash): (f32, &[f32]) = match join {
      Join::Reset => continue,
      Join::Gap => (GAP_LINE_WIDTH, &DOT),
      Join::Solid if line.dashed => (LINE_WIDTH, &DASH),
      Join::Solid => (LINE_WIDTH, &[]),
    };
    context.set_line_width(width);
    context.set_line_dash(dash);
    context.begin_path();
    context.move_to(plot.x(frame, pair[0].at), frame.y(pair[0].percent, 100.0));
    context.line_to(plot.x(frame, pair[1].at), frame.y(pair[1].percent, 100.0));
    context.stroke();
  }
  context.set_line_dash(&[]);
}

fn dot(context: &Context2D, (x, y): (f32, f32), color: lurq::node::color::Color) {
  context.set_fill_style(color);
  context.begin_path();
  // A full circle is always valid geometry; one the canvas refuses is not drawn.
  if context
    .arc(x, y, POINT_RADIUS, 0.0, TAU, ArcDirection::Clockwise)
    .is_ok()
  {
    context.fill();
  }
}

/// The dashed warning line and its value at its end.
fn threshold(context: &Context2D, frame: Frame, plot: &LimitPlot) {
  let y = frame.y(f64::from(plot.threshold), 100.0).round() + 0.5;
  context.set_stroke_style(plot.style.threshold);
  context.set_line_width(RULE_WIDTH);
  context.set_line_dash(&DASH);
  context.begin_path();
  context.move_to(frame.left, y);
  context.line_to(frame.right, y);
  context.stroke();
  context.set_line_dash(&[]);
  context.set_font(plot.style.font.clone());
  context.set_text_align(TextAlign::Right);
  context.set_text_baseline(TextBaseline::Bottom);
  context.set_fill_style(plot.style.threshold);
  plot::text(context, &plot.threshold_label, frame.right, y - GAP_LINE_WIDTH);
}

/// 45° lines over each stretch before a CLI's first report.
fn hatch(context: &Context2D, frame: Frame, plot: &LimitPlot) {
  context.set_stroke_style(plot.style.hatch);
  context.set_line_width(GAP_LINE_WIDTH);
  context.set_line_dash(&[]);
  for (from, to) in &plot.hatches {
    let (left, right) = (
      plot.x(frame, *from).max(frame.left),
      plot.x(frame, *to).min(frame.right),
    );
    if right <= left {
      continue;
    }
    context.save();
    context.begin_path();
    context.rect(left, frame.top, right - left, frame.height());
    context.clip();
    let mut x = left - frame.height();
    while x < right {
      context.begin_path();
      context.move_to(x, frame.bottom);
      context.line_to(x + frame.height(), frame.top);
      context.stroke();
      x += HATCH_PITCH;
    }
    context.restore();
  }
}
