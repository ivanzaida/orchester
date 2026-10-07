//! What the dashboard's canvas charts share: the plot's frame inside the canvas, a scale with round
//! steps, the grid with its axis labels, the bars of the x axis, and the redraw when a canvas
//! changes size.

use std::sync::Arc;

use lurq::canvas::{CanvasHandle, CanvasId, CanvasObserver, Context2D, TextAlign, TextBaseline};
use parking_lot::Mutex;

use crate::theme::chart::{AXIS_BAND, AXIS_GUTTER, ChartStyle, LABEL_GAP, PLOT_TOP as TOP, RULE_WIDTH};

/// Grid lines above the baseline.
const TICKS: u64 = 4;

/// Redraws a chart whenever its canvas changes size. When the page moves the chart (beside another
/// card on a wide window, under it on a narrow one) lurq gives it a new canvas: that one is bound in
/// turn and drawn at once.
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

/// The plot's rectangle inside its canvas: the y axis labels at the left, the x axis labels below.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
  pub left: f32,
  pub top: f32,
  pub right: f32,
  pub bottom: f32,
}

impl Frame {
  pub fn of(canvas: &CanvasHandle) -> Self {
    let size = canvas.size();
    Self {
      left: AXIS_GUTTER,
      top: TOP,
      right: size.width.max(AXIS_GUTTER + 1.0),
      bottom: (size.height - AXIS_BAND).max(TOP + 1.0),
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

/// A scale whose top is a round number at least `max`, in [`TICKS`] equal steps.
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
/// baseline.
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
    let y = frame.y(value as f64, top as f64).round() + 0.5;
    context.set_stroke_style(if value == 0 { style.rule } else { style.grid });
    context.begin_path();
    context.move_to(frame.left, y);
    context.line_to(frame.right, y);
    context.stroke();
    context.set_fill_style(style.axis);
    text(context, &label(value), frame.left - LABEL_GAP, y);
    value += step.max(1);
  }
}

/// An x axis label centered under `x`.
pub fn x_label(context: &Context2D, frame: Frame, style: &ChartStyle, label: &str, x: f32) {
  context.set_font(style.font.clone());
  context.set_text_align(TextAlign::Center);
  context.set_text_baseline(TextBaseline::Top);
  context.set_fill_style(style.axis);
  text(context, label, x, frame.bottom + LABEL_GAP);
}

/// Draws `label`; text that the canvas cannot draw yet (its text service is not attached) is left
/// out of this frame and drawn with the next.
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
