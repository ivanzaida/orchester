//! The *Cache share* line of *Efficiency* (`C2pTYF`): one line in `$muted` over the days, a direct
//! label on the last point; a day without input has no share and breaks the line rather than
//! inventing one.

use std::{f32::consts::TAU, sync::Arc};

use lurq::{
  app::{component::Component, ctx::Ctx},
  canvas::{ArcDirection, CanvasHandle, CanvasItem, TextAlign, TextBaseline},
  components::Canvas,
  core::ElementRef,
  node::{Element, dimension::Dimension},
};
use parking_lot::Mutex;

use super::plot::{self, AxisName, AxisText, Frame, Redraw, XLabel};
use crate::theme::chart::{ChartStyle, HEADROOM, LABEL_GAP, LINE_WIDTH, POINT_RADIUS, SMALL_PLOT_HEIGHT};

/// One day: its axis label, its share in percent if known, and its name and value for its canvas
/// item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Day {
  pub label: Option<AxisName>,
  pub share: Option<u64>,
  pub name: Arc<str>,
  pub value: Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineProps {
  pub days: Arc<[Day]>,
  /// The y axis labels at 0, 25, 50, 75 and 100 %.
  pub percents: [Arc<str>; 5],
}

impl lurq::app::component::DevtoolsInspectable for LineProps {}

#[derive(Clone)]
struct LinePlot {
  props: LineProps,
  style: ChartStyle,
}

pub struct CacheLine {
  reference: ElementRef,
  plot: Arc<Mutex<Option<LinePlot>>>,
  redraw: Redraw,
}

impl Component for CacheLine {
  type Props = LineProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      reference: ctx.element_ref(),
      plot: Arc::new(Mutex::new(None)),
      redraw: Redraw::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let plot = LinePlot {
      props: ctx.props::<LineProps>().clone(),
      style: ChartStyle::of(ctx),
    };
    if let Some(canvas) = self.reference.as_canvas().filter(CanvasHandle::is_attached) {
      draw(&canvas, &plot);
    }
    *self.plot.lock() = Some(plot);
    Canvas::new()
      .ref_element(self.reference.clone())
      .id("usage-cache-chart")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(SMALL_PLOT_HEIGHT))
  }

  fn after_layout(&self) {
    if let Some(canvas) = self.reference.as_canvas() {
      self.redraw.bind(&canvas, &self.plot, draw);
    }
  }
}

fn draw(canvas: &CanvasHandle, plot: &LinePlot) {
  let context = canvas.context_2d();
  context.reset();
  let frame = Frame::of(canvas);
  let frame = Frame {
    top: frame.bottom - frame.height() * HEADROOM,
    ..frame
  };
  let days = &plot.props.days;
  plot::grid(&context, frame, &plot.style, (100, 50), |value| {
    plot.props.percents[usize::try_from(value / 25).unwrap_or(0).min(4)].to_string()
  });
  let point = |index: usize, share: u64| {
    let (slot, width) = frame.slot(index, days.len());
    (slot + width / 2.0, frame.y(share as f64, 100.0))
  };
  context.set_stroke_style(plot.style.single);
  context.set_line_width(LINE_WIDTH);
  context.set_line_dash(&[]);
  let mut items = Vec::new();
  let mut previous: Option<(f32, f32)> = None;
  for (index, day) in days.iter().enumerate() {
    if let Some(name) = &day.label {
      let (slot, width) = frame.slot(index, days.len());
      let label = XLabel {
        name,
        kind: AxisText::Day,
        x: slot + width / 2.0,
      };
      items.extend(plot::x_label(&context, frame, &plot.style, &label, index));
    }
    let Some(share) = day.share else {
      previous = None;
      continue;
    };
    let (x, y) = point(index, share);
    if let Some((from_x, from_y)) = previous {
      context.begin_path();
      context.move_to(from_x, from_y);
      context.line_to(x, y);
      context.stroke();
    }
    previous = Some((x, y));
    items.push(
      CanvasItem::point(format!("cache-{index}"), "share", x, y, POINT_RADIUS * 2.0)
        .label(day.name.as_ref())
        .value(day.value.as_ref()),
    );
  }
  let last = days
    .iter()
    .enumerate()
    .rev()
    .find_map(|(index, day)| Some((index, day.share?, day)));
  if let Some((index, share, day)) = last {
    let (x, y) = point(index, share);
    context.set_fill_style(plot.style.single);
    context.begin_path();
    // A full circle is always valid geometry; one the canvas refuses is not drawn.
    if context
      .arc(x, y, POINT_RADIUS, 0.0, TAU, ArcDirection::Clockwise)
      .is_ok()
    {
      context.fill();
    }
    context.set_font(plot.style.font.clone());
    context.set_text_align(TextAlign::Right);
    context.set_text_baseline(TextBaseline::Bottom);
    plot::text(&context, &day.value, x, y - LABEL_GAP);
  }
  canvas.set_items(items);
}
