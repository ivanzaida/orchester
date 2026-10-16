//! Draws *Tokens over time* (`Plot` `cgGn2`): one bar per hour or day, stacked by account series from the
//! bottom (accounts 1–3, Other accounts, No account), 24 wide at most, the parts 2 apart and the top one
//! rounded 4 (`Bar` `umBAa`); No account's part hatched, `chart_none` stripes and outline over `chart_hatch`
//! (`LEQ1w`); on a grid in compact tokens; the hovered bar's slot gets the `hover` band. Each bar is a canvas
//! item too, so automation and screen readers find and read it.

use std::sync::Arc;

use lurq::canvas::{CanvasHandle, CanvasItem, Context2D};
use orchester_core::{
  tokens::compact_tokens,
  usage::{AccountSeries, SERIES},
};

use super::plot::{self, AxisName, AxisText, Frame, XLabel};
use crate::theme::usage::ChartStyle;

/// A bar's widest (`Bar` `umBAa`), and its share of a slot narrower than that allows.
const BAR_WIDTH: f32 = 24.0;
const BAR_FILL: f32 = 0.6;
/// Between the stacked parts of a bar, and the rounding of its top.
const BAR_GAP: f32 = 2.0;
const BAR_RADIUS: f32 = 4.0;
/// No account's stripes (`LEQ1w`): 1 wide, 4 apart.
const STRIPE_WIDTH: f32 = 1.0;
const STRIPE_PITCH: f32 = 4.0;

/// What the chart draws.
#[derive(Clone)]
pub struct TokenPlot {
  /// Each bar's parts, bottom first, by account series (`AccountSeries::index`).
  pub bars: Vec<[u64; SERIES]>,
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
  parts: &[u64; SERIES],
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
    let none = *series == AccountSeries::NoAccount.index();
    let rounded = position + 1 == shown.len();
    context.set_fill_style(if none { style.hatch } else { style.series[*series] });
    part_path(context, (x, y, width, height), rounded);
    context.fill();
    if none {
      hatch_part(context, (x, y, width, height), rounded, style.series[*series]);
    }
    base = y;
  }
  base
}

/// No account's stripes, 45° and `STRIPE_PITCH` apart, and its 1 outline, over its ground; both inside the
/// part's rounded top when it is the bar's top part.
fn hatch_part(
  context: &Context2D,
  (x, y, width, height): (f32, f32, f32, f32),
  rounded: bool,
  stripe: lurq::node::color::Color,
) {
  context.save();
  part_path(context, (x, y, width, height), rounded);
  context.clip();
  context.set_stroke_style(stripe);
  context.set_line_width(STRIPE_WIDTH);
  context.set_line_dash(&[]);
  let mut from = x - height;
  while from < x + width {
    context.begin_path();
    context.move_to(from, y + height);
    context.line_to(from + height, y);
    context.stroke();
    from += STRIPE_PITCH;
  }
  context.restore();
  let inset = STRIPE_WIDTH / 2.0;
  part_path(
    context,
    (x + inset, y + inset, width - STRIPE_WIDTH, height - STRIPE_WIDTH),
    rounded,
  );
  context.stroke();
}

/// A bar part's path: a rectangle, with its top corners rounded when it is the bar's top part. Geometry the
/// canvas refuses (a bar thinner than a pixel) is a plain rectangle.
fn part_path(context: &Context2D, (x, y, width, height): (f32, f32, f32, f32), rounded: bool) {
  context.begin_path();
  let radius = BAR_RADIUS.min(width / 2.0).min(height);
  if rounded && radius > 0.0 {
    context.move_to(x, y + height);
    let corners = context
      .arc_to(x, y, x + width, y, radius)
      .and_then(|()| context.arc_to(x + width, y, x + width, y + height, radius));
    if corners.is_ok() {
      context.line_to(x + width, y + height);
      context.close_path();
      return;
    }
    context.begin_path();
  }
  context.rect(x, y, width, height);
}
