//! Draws *Tokens over time* (`Plot` `cgGn2`): one bar per hour or day, stacked by CLI from the bottom
//! (Codex, Claude Code, another CLI), 24 wide at most, the parts 2 apart and the top one rounded 4
//! (`Bar` `umBAa`), on a grid in compact tokens; the hovered bar's slot gets the `hover` band. Each bar is a
//! canvas item too, so automation and screen readers find and read it.

use std::sync::Arc;

use lurq::canvas::{CanvasHandle, CanvasItem, Context2D};
use orchester_core::tokens::compact_tokens;

use super::plot::{self, AxisName, AxisText, Frame, XLabel};
use crate::theme::usage::ChartStyle;

/// A bar's widest (`Bar` `umBAa`), and its share of a slot narrower than that allows.
const BAR_WIDTH: f32 = 24.0;
const BAR_FILL: f32 = 0.6;
/// Between the stacked parts of a bar, and the rounding of its top.
const BAR_GAP: f32 = 2.0;
const BAR_RADIUS: f32 = 4.0;

/// What the chart draws.
#[derive(Clone)]
pub struct TokenPlot {
  /// Each bar's parts, bottom first, by CLI series.
  pub bars: Vec<[u64; 3]>,
  /// Each bar's x axis label; `None` where labels would crowd.
  pub labels: Vec<Option<AxisName>>,
  /// What the labels name: hours of one day, or days.
  pub label_kind: AxisText,
  /// Each bar's name and exact total, for its canvas item.
  pub items: Vec<(Arc<str>, Arc<str>)>,
  pub hovered: Option<usize>,
  pub style: ChartStyle,
}

pub fn draw(canvas: &CanvasHandle, plot: &TokenPlot) {
  let context = canvas.context_2d();
  context.reset();
  let frame = Frame::of(canvas, 0.0);
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
    let bar_width = (width * BAR_FILL).clamp(1.0, BAR_WIDTH);
    let x = slot + (width - bar_width) / 2.0;
    let top = stacked(&context, (x, bar_width), frame, bar, (scale.0, &plot.style));
    if let Some(name) = &plot.labels[index] {
      let label = XLabel {
        name,
        kind: plot.label_kind,
        x: slot + width / 2.0,
      };
      items.extend(plot::x_label(&context, frame, &plot.style, &label, index));
    }
    let (name, value) = &plot.items[index];
    let top = top.min(frame.bottom);
    items.push(
      CanvasItem::rect(format!("bar-{index}"), "bar", slot, top, width, frame.bottom - top)
        .label(name.as_ref())
        .value(value.as_ref()),
    );
  }
  canvas.set_items(items);
}

/// The bar's parts from the baseline up, each but the first `BAR_GAP` above the one below; the top part is
/// rounded. The y of its top.
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
  // Bars reach the plot's bottom, over the baseline (`Bar` `umBAa`).
  let mut base = frame.bottom;
  for (position, (series, value)) in shown.iter().enumerate() {
    let height = frame.bottom - frame.y(*value as f64, top as f64);
    let gap = if position > 0 { BAR_GAP } else { 0.0 };
    let y = base - gap - height;
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
