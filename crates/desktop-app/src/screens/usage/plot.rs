//! What the Usage page's canvas charts share (`Chart` `nySdK`): the plot's frame inside its canvas (the y
//! axis's 34 and 8 at its left, a 190 high plot, the x axis's labels 6 under it), a scale with round steps,
//! the grid with its labels, the x axis's labels, and the redraw when a canvas changes size.

use std::sync::Arc;

use chrono::NaiveDate;
use lurq::{
  app::ctx::Ctx,
  canvas::{CanvasHandle, CanvasId, CanvasItem, CanvasObserver, Context2D, TextAlign, TextBaseline},
};
use parking_lot::Mutex;

use super::words;
use crate::theme::usage::ChartStyle;

/// The y axis's labels and the gap after them (`Y axis` `t2E26`, `Plot row` `K6zIl`).
pub const AXIS_GUTTER: f32 = 42.0;
/// Room over the plot for its top label, which is centered on the top grid line (`Tick 8M` `Quv96`).
pub const TOP_ROOM: f32 = 7.0;
/// The plot of *Tokens over time* and *Plan limits*, from its top grid line to its baseline (`Plot` `cgGn2`).
pub const PLOT_HEIGHT: f32 = 190.0;
/// Between the plot and the x axis's labels, and their line (`Chart` `nySdK`, `X labels` `y5aeqR`).
pub const LABEL_GAP: f32 = 6.0;
pub const LABEL_LINE: f32 = 13.0;
/// A chart's canvas: the room over the plot, the plot, and its x axis.
pub const CHART_HEIGHT: f32 = TOP_ROOM + PLOT_HEIGHT + LABEL_GAP + LABEL_LINE;
/// Between a y axis label and the plot.
const TICK_GAP: f32 = 8.0;
/// Grid lines above the baseline.
const TICKS: u64 = 4;
/// The width of a rule and a grid line.
pub const RULE_WIDTH: f32 = 1.0;

/// Redraws a chart whenever its canvas changes size. When the page moves the chart lurq gives it a new
/// canvas: that one is bound in turn and drawn at once.
#[derive(Default)]
pub struct Redraw(Mutex<Option<(CanvasId, CanvasObserver)>>);

impl Redraw {
  pub fn bind<P: Send + 'static>(
    &self,
    canvas: &CanvasHandle,
    plot: &Arc<Mutex<Option<P>>>,
    draw: fn(&CanvasHandle, &P),
  ) {
    let mut bound = self.0.lock();
    if bound.as_ref().is_some_and(|(id, _)| *id == canvas.surface_id()) {
      return;
    }
    let (target, shared) = (canvas.clone(), Arc::clone(plot));
    let observer = canvas.observe_metrics(move |_| {
      if let Some(plot) = shared.lock().as_ref() {
        draw(&target, plot);
      }
    });
    if canvas.is_attached()
      && let Some(plot) = plot.lock().as_ref()
    {
      draw(canvas, plot);
    }
    *bound = Some((canvas.surface_id(), observer));
  }
}

/// The plot's rectangle inside its canvas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
  pub left: f32,
  pub top: f32,
  pub right: f32,
  pub bottom: f32,
  /// The canvas's right edge, which labels stay inside.
  pub edge: f32,
}

impl Frame {
  /// The plot of `canvas`, `right_room` short of its right edge (*Plan limits* keeps 30 for its lines'
  /// direct labels, `bsp8a`).
  pub fn of(canvas: &CanvasHandle, right_room: f32) -> Self {
    let size = canvas.size();
    Self {
      left: AXIS_GUTTER,
      top: TOP_ROOM,
      right: (size.width - right_room).max(AXIS_GUTTER + 1.0),
      bottom: (size.height - LABEL_GAP - LABEL_LINE).max(TOP_ROOM + 1.0),
      edge: size.width,
    }
  }

  pub fn width(&self) -> f32 {
    self.right - self.left
  }

  pub fn height(&self) -> f32 {
    self.bottom - self.top
  }

  /// The y of `value` on a scale up to `top`, which is the plot's top.
  pub fn y(&self, value: f64, top: f64) -> f32 {
    if top <= 0.0 {
      return self.bottom;
    }
    self.bottom - (value / top) as f32 * self.height()
  }

  /// The left edge and width of slot `index` of `count`, as bars and hover bands use them.
  pub fn slot(&self, index: usize, count: usize) -> (f32, f32) {
    let width = self.width() / count.max(1) as f32;
    (self.left + width * index as f32, width)
  }

  /// The slot under `x`, if any.
  pub fn slot_at(&self, x: f32, count: usize) -> Option<usize> {
    if count == 0 || x < self.left || x >= self.right {
      return None;
    }
    let index = ((x - self.left) / (self.width() / count as f32)) as usize;
    (index < count).then_some(index)
  }
}

/// A scale whose top is a round number at least `max`, in [`TICKS`] equal steps: its top and its step.
pub fn scale(max: u64) -> (u64, u64) {
  if max == 0 {
    return (TICKS, 1);
  }
  let rough = max.div_ceil(TICKS);
  let magnitude = 10_u64.pow(rough.ilog10());
  let step = [1, 2, 5, 10]
    .into_iter()
    .map(|factor| factor * magnitude)
    .find(|step| *step >= rough)
    .unwrap_or(10 * magnitude);
  (step * TICKS, step)
}

/// The grid lines of a scale up to `top` in `step`s, each labelled by `label` at the left, and the
/// baseline in `border_strong`.
pub fn grid(
  context: &Context2D,
  frame: Frame,
  style: &ChartStyle,
  (top, step): (u64, u64),
  label: impl Fn(u64) -> String,
) {
  context.set_line_width(RULE_WIDTH);
  context.set_line_dash(&[]);
  context.set_font(style.font.clone());
  context.set_text_align(TextAlign::Right);
  context.set_text_baseline(TextBaseline::Middle);
  let mut value = 0;
  while value <= top {
    let y = frame.y(value as f64, top as f64).round() - 0.5;
    context.set_stroke_style(if value == 0 { style.baseline } else { style.grid });
    context.begin_path();
    context.move_to(frame.left, y);
    context.line_to(frame.right, y);
    context.stroke();
    context.set_fill_style(style.axis);
    text(context, &label(value), frame.left - TICK_GAP, y);
    value += step.max(1);
  }
}

/// What an x axis label names, which picks its font: a clock time in mono, a day in sans.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxisText {
  Clock,
  Day,
}

/// An x axis label's text, and whether it names today, which reads brighter than the other days
/// (`X Today` `bOe4q`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AxisName {
  pub text: Arc<str>,
  pub today: bool,
}

impl AxisName {
  pub fn plain(text: Arc<str>) -> Self {
    Self { text, today: false }
  }

  /// `date` by its weekday, or *Today*.
  pub fn day(ctx: &Ctx, date: NaiveDate, today: NaiveDate) -> Self {
    if date == today {
      Self {
        text: ctx.t("usage.date.today"),
        today: true,
      }
    } else {
      Self::plain(words::weekday(ctx, date))
    }
  }
}

/// An x axis label to draw: its name, what it names, and the x it is centered under.
pub struct XLabel<'a> {
  pub name: &'a AxisName,
  pub kind: AxisText,
  pub x: f32,
}

/// Draws an x axis label centered under its x, kept whole inside the canvas. Its canvas item,
/// `x-label-{index}`, names it where it was drawn. A label the canvas cannot measure yet (its text service
/// is not attached) waits for the next frame.
pub fn x_label(
  context: &Context2D,
  frame: Frame,
  style: &ChartStyle,
  label: &XLabel,
  index: usize,
) -> Option<CanvasItem> {
  let font = match (label.kind, label.name.today) {
    (_, true) => style.strong_font.clone(),
    (AxisText::Clock, false) => style.font.clone(),
    (AxisText::Day, false) => style.day_font.clone(),
  };
  context.set_font(font);
  context.set_text_align(TextAlign::Left);
  context.set_text_baseline(TextBaseline::Top);
  context.set_fill_style(if label.name.today { style.strong } else { style.axis });
  let words = &*label.name.text;
  let width = match context.measure_text(words) {
    Ok(metrics) => metrics.width,
    Err(error) => {
      tracing::debug!(%error, "a chart label waits for the canvas text service");
      return None;
    }
  };
  let left = (label.x - width / 2.0).min(frame.edge - width).max(0.0);
  let top = frame.bottom + LABEL_GAP;
  text(context, words, left, top);
  Some(CanvasItem::rect(format!("x-label-{index}"), "label", left, top, width, LABEL_LINE).label(words))
}

/// Draws `label`; text that the canvas cannot draw yet (its text service is not attached) is left out of
/// this frame and drawn with the next.
pub fn text(context: &Context2D, label: &str, x: f32, y: f32) {
  if let Err(error) = context.fill_text(label, x, y) {
    tracing::debug!(%error, "a chart label waits for the canvas text service");
  }
}

/// Every `n`th label, so labels of many bars do not run into each other.
pub fn label_every(count: usize, width: f32, label_width: f32) -> usize {
  let fits = (width / label_width).floor().max(1.0) as usize;
  count.div_ceil(fits).max(1)
}
