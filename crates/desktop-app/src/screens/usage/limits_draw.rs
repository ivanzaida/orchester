//! Draws *Plan limits* (`Plot` `jPHZ9`): % used per window over the range, solid 2 wide for weekly windows
//! and dashed for 5-hour ones, in the CLI's color. Reports that kept coming are joined; a stretch without
//! reports is a dotted 1.5 join; a window that reset is not joined across its reset, which gets a
//! `border_strong` rule and a reset marker over the plot; before a CLI's first report the plot is hatched
//! and says so. Each window's last point is ringed in `surface_panel` with the CLI's name beside it. The
//! warning threshold is dashed in `text_muted`, its value at its end, in the 30 the plot leaves at its
//! right for its labels.

use std::{f32::consts::TAU, sync::Arc};

use lurq::canvas::{ArcDirection, CanvasFont, CanvasHandle, CanvasItem, Context2D, TextAlign, TextBaseline};
use orchester_core::usage::limits::Join;

use super::plot::{self, AxisName, AxisText, Frame, RULE_WIDTH, XLabel};
use crate::theme::usage::ChartStyle;

/// What the plot leaves at its right for the lines' names and the threshold's value (`Plot area` `bsp8a`).
pub const RIGHT_ROOM: f32 = 30.0;
const LINE_WIDTH: f32 = 2.0;
const GAP_LINE_WIDTH: f32 = 1.5;
const DASH: [f32; 2] = [4.0, 3.0];
const DOT: [f32; 2] = [1.5, 3.0];
/// A window's last point (`Codex · weekly · current` `fPyMV`): 8 across in a 2 ring.
const POINT_RADIUS: f32 = 4.0;
const RING_WIDTH: f32 = 2.0;
/// A hovered point's mark, and how near the pointer finds a point.
const HIT_SIZE: f32 = 12.0;
/// Between a point or a line's end and its label, and a marker over the plot (`HeWml`, `mcq7x`).
const LABEL_GAP: f32 = 6.0;
const MARKER_TOP: f32 = 2.0;
const MARKER_SIZE: f32 = 12.0;
const HATCH_PITCH: f32 = 6.0;
/// The hatch's label from the plot's bottom left (`Hatch label` `dpVZL`).
const HATCH_LABEL_INSET: f32 = 4.0;

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
  /// The CLI's name beside the last point.
  pub label: Arc<str>,
}

#[derive(Clone)]
pub struct LimitPlot {
  pub start: i64,
  pub end: i64,
  pub lines: Vec<DrawnLine>,
  /// When a weekly window reset, with its canvas item's name.
  pub resets: Vec<(i64, Arc<str>)>,
  /// Stretches before a CLI's first report, each with what it says (*No Claude data*).
  pub hatches: Vec<(i64, i64, Arc<str>)>,
  pub threshold: u8,
  pub threshold_label: Arc<str>,
  /// The y axis labels at 0, 25, 50, 75 and 100 %.
  pub percents: [Arc<str>; 5],
  /// x axis labels at their times, and what they name: hours of one day, or days.
  pub labels: Vec<(i64, AxisName)>,
  pub label_kind: AxisText,
  /// The hovered point: its line and index.
  pub hovered: Option<(usize, usize)>,
  /// The reset marker's glyph and its font.
  pub marker: (Arc<str>, CanvasFont),
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
  let frame = Frame::of(canvas, RIGHT_ROOM);
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
  items.extend(resets(&context, frame, plot));
  threshold(&context, frame, plot);
  for (line_index, line) in plot.lines.iter().enumerate() {
    draw_line(&context, frame, plot, line);
    for (point_index, point) in line.points.iter().enumerate() {
      let (x, y) = (plot.x(frame, point.at), frame.y(point.percent, 100.0));
      if plot.hovered == Some((line_index, point_index)) {
        dot(&context, (x, y), plot.style.series[line.series], &plot.style);
      }
      items.push(
        CanvasItem::point(format!("limit-{line_index}-{point_index}"), "limit", x, y, HIT_SIZE)
          .label(point.name.as_ref())
          .value(point.value.as_ref()),
      );
    }
    current(&context, frame, plot, line);
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

/// The window's last point, ringed, with the CLI's name after it, or before it where the room after it up to
/// the canvas's edge is too narrow (a last reading late in the day).
fn current(context: &Context2D, frame: Frame, plot: &LimitPlot, line: &DrawnLine) {
  let Some(last) = line.points.last() else {
    return;
  };
  let (x, y) = (plot.x(frame, last.at), frame.y(last.percent, 100.0));
  dot(context, (x, y), plot.style.series[line.series], &plot.style);
  context.set_font(plot.style.strong_font.clone());
  context.set_text_baseline(TextBaseline::Middle);
  context.set_fill_style(plot.style.strong);
  let after = x + POINT_RADIUS + LABEL_GAP;
  let fits_after = context
    .measure_text(&line.label)
    .map_or(true, |metrics| after + metrics.width <= frame.edge);
  if fits_after {
    context.set_text_align(TextAlign::Left);
    plot::text(context, &line.label, after, y);
  } else {
    context.set_text_align(TextAlign::Right);
    plot::text(context, &line.label, x - POINT_RADIUS - LABEL_GAP, y);
  }
}

/// A point in its series' color, ringed in the card's surface.
fn dot(context: &Context2D, (x, y): (f32, f32), color: lurq::node::color::Color, style: &ChartStyle) {
  context.begin_path();
  // A full circle is always valid geometry; one the canvas refuses is not drawn.
  if context
    .arc(x, y, POINT_RADIUS, 0.0, TAU, ArcDirection::Clockwise)
    .is_ok()
  {
    context.set_fill_style(color);
    context.fill();
    context.set_stroke_style(style.ring);
    context.set_line_width(RING_WIDTH);
    context.set_line_dash(&[]);
    context.stroke();
  }
}

/// Each weekly reset: a rule from under its marker to the baseline, and the marker.
fn resets(context: &Context2D, frame: Frame, plot: &LimitPlot) -> Vec<CanvasItem> {
  let mut items = Vec::new();
  let (glyph, font) = &plot.marker;
  for (index, (at, name)) in plot.resets.iter().enumerate() {
    let x = plot.x(frame, *at).round() + 0.5;
    let rule_top = frame.top + MARKER_TOP + MARKER_SIZE + MARKER_TOP;
    context.set_stroke_style(plot.style.baseline);
    context.set_line_width(RULE_WIDTH);
    context.set_line_dash(&[]);
    context.begin_path();
    context.move_to(x, rule_top);
    context.line_to(x, frame.bottom);
    context.stroke();
    context.set_font(font.clone());
    context.set_text_align(TextAlign::Center);
    context.set_text_baseline(TextBaseline::Top);
    context.set_fill_style(plot.style.axis);
    plot::text(context, glyph, x, frame.top + MARKER_TOP);
    items.push(
      CanvasItem::rect(
        format!("reset-{index}"),
        "reset",
        x - HIT_SIZE / 2.0,
        frame.top,
        HIT_SIZE,
        frame.height(),
      )
      .label(name.as_ref()),
    );
  }
  items
}

/// The dashed warning line and its value at its end, in the plot's right room.
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
  context.set_text_align(TextAlign::Left);
  context.set_text_baseline(TextBaseline::Middle);
  context.set_fill_style(plot.style.threshold);
  plot::text(context, &plot.threshold_label, frame.right + MARKER_TOP * 2.0, y);
}

/// 45° lines over each stretch before a CLI's first report, and what it says at its bottom left where it
/// fits: hatches all start with the range, so a narrower one's label would run over a wider one's (the
/// footnote names every stretch).
fn hatch(context: &Context2D, frame: Frame, plot: &LimitPlot) {
  context.set_stroke_style(plot.style.hatch);
  context.set_line_width(RULE_WIDTH);
  context.set_line_dash(&[]);
  for (from, to, label) in &plot.hatches {
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
    context.set_font(plot.style.day_font.clone());
    context.set_text_align(TextAlign::Left);
    context.set_text_baseline(TextBaseline::Bottom);
    context.set_fill_style(plot.style.axis);
    let fits = context
      .measure_text(label)
      .is_ok_and(|metrics| metrics.width + 2.0 * HATCH_LABEL_INSET <= right - left);
    if fits {
      plot::text(
        context,
        label,
        left + HATCH_LABEL_INSET,
        frame.bottom - HATCH_LABEL_INSET,
      );
    }
  }
}
