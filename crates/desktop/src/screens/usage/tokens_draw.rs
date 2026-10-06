//! Draws *Tokens over time* (`C2pTYF`): one bar per hour or day, stacked by CLI from the bottom
//! (Codex, Claude Code, another CLI) with a 2px gap and a rounded top, on a grid in compact tokens;
//! the hovered bar's slot gets a band. Each bar is also a canvas item, so automation and screen
//! readers can find and read it.

use std::sync::Arc;

use lurq::canvas::{CanvasHandle, CanvasItem, Context2D};
use orchester_core::tokens::compact_tokens;

use super::plot::{self, Frame};
use crate::theme::chart::{BAR_FILL, BAR_GAP, BAR_RADIUS, ChartStyle};

/// What the chart draws.
#[derive(Clone)]
pub struct TokenPlot {
  /// Each bar's parts, bottom first, by CLI series.
  pub bars: Vec<[u64; 3]>,
  /// Each bar's x axis label; `None` where labels would crowd.
  pub labels: Vec<Option<Arc<str>>>,
  /// Each bar's name and exact total, for its canvas item.
  pub items: Vec<(Arc<str>, Arc<str>)>,
  pub hovered: Option<usize>,
  pub style: ChartStyle,
}

pub fn draw(canvas: &CanvasHandle, plot: &TokenPlot) {
  let context = canvas.context_2d();
  context.reset();
  let frame = Frame::of(canvas);
  let max = plot.bars.iter().map(|bar| bar.iter().sum::<u64>()).max().unwrap_or(0);
  let scale = plot::scale(max);
  if let Some(index) = plot.hovered {
    let (x, width) = frame.slot(index, plot.bars.len());
    context.set_fill_style(plot.style.band);
    context.fill_rect(x, frame.top, width, frame.height());
  }
  plot::grid(&context, frame, &plot.style, scale, compact_tokens);
  let mut items = Vec::with_capacity(plot.bars.len());
  for (index, bar) in plot.bars.iter().enumerate() {
    let (slot, width) = frame.slot(index, plot.bars.len());
    let bar_width = (width * BAR_FILL).max(1.0);
    let x = slot + (width - bar_width) / 2.0;
    let top = stacked(&context, (x, bar_width), frame, bar, (scale.0, &plot.style));
    if let Some(label) = &plot.labels[index] {
      plot::x_label(&context, frame, &plot.style, label, slot + width / 2.0);
    }
    let (name, value) = &plot.items[index];
    items.push(
      CanvasItem::rect(
        format!("bar-{index}"),
        "bar",
        slot,
        top.min(frame.bottom),
        width,
        frame.bottom - top.min(frame.bottom),
      )
      .label(name.as_ref())
      .value(value.as_ref()),
    );
  }
  canvas.set_items(items);
}

/// The bar's parts from the baseline up, each but the first `BAR_GAP` above the one below; the top
/// part is rounded. The y of its top.
fn stacked(
  context: &Context2D,
  (x, width): (f32, f32),
  frame: Frame,
  parts: &[u64; 3],
  (top, style): (u64, &ChartStyle),
) -> f32 {
  let shown: Vec<(usize, u64)> = parts
    .iter()
    .copied()
    .enumerate()
    .filter(|(_, value)| *value > 0)
    .collect();
  let mut base = frame.bottom;
  for (position, (series, value)) in shown.iter().enumerate() {
    let height = frame.bottom - frame.y(*value as f64, top as f64);
    let gap = if position > 0 { BAR_GAP } else { 0.0 };
    let (y, height) = (base - gap - height, height);
    context.set_fill_style(style.series[*series]);
    if position + 1 == shown.len() {
      rounded_top(context, x, y, width, height);
    } else {
      context.fill_rect(x, y, width, height);
    }
    base = y;
  }
  base
}

/// A rectangle whose top corners are rounded.
fn rounded_top(context: &Context2D, x: f32, y: f32, width: f32, height: f32) {
  let radius = BAR_RADIUS.min(width / 2.0).min(height);
  context.begin_path();
  // Geometry the canvas refuses (a bar thinner than a pixel) is drawn square, by the rect below.
  if context.round_rect(x, y, width, height, radius).is_ok() {
    context.fill();
  }
  if height > radius {
    context.fill_rect(x, y + radius, width, height - radius);
  }
}
