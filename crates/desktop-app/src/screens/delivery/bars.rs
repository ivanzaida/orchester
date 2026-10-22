//! A bar chart of the Delivery view on a canvas, drawn with the Usage page's chart pieces (`screens::usage::plot`):
//! the grid and its labels, one bar per slot in the first series color with its x label, and, where the chart has
//! one, a dashed reference line across the plot in `text_muted` (a median). Each bar is a canvas item with its name
//! and value, so the window's tree reads what is drawn.

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
  screens::usage::plot::{self, AxisName, AxisText, CHART_HEIGHT, Frame, Redraw, XLabel},
  theme::usage::ChartStyle,
};

/// How much of its slot a bar is wide, and the widest it gets.
const BAR_SHARE: f32 = 0.6;
const BAR_MAX: f32 = 28.0;
const LINE_WIDTH: f32 = 1.5;
/// The reference line's dash (as the burndown's ideal line is dashed).
const DASH: [f32; 2] = [4.0, 3.0];

/// One bar: its x label (`None`: left out so labels do not run together), its height, and how it reads.
#[derive(Clone, Debug, PartialEq)]
pub struct Bar {
  pub label: Option<AxisName>,
  pub value: f64,
  pub name: Arc<str>,
  pub text: Arc<str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BarsProps {
  /// The canvas's id.
  pub id: &'static str,
  pub bars: Arc<[Bar]>,
  /// Where a dashed line goes across the plot.
  pub reference: Option<f64>,
}

impl DevtoolsInspectable for BarsProps {}

#[derive(Clone)]
struct Plot {
  props: BarsProps,
  style: ChartStyle,
}

pub struct BarChart {
  reference: ElementRef,
  plot: Arc<Mutex<Option<Plot>>>,
  redraw: Redraw,
}

impl Component for BarChart {
  type Props = BarsProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      reference: ctx.element_ref(),
      plot: Arc::new(Mutex::new(None)),
      redraw: Redraw::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<BarsProps>().clone();
    let id = props.id;
    let plot = Plot {
      props,
      style: ChartStyle::of(ctx),
    };
    if let Some(canvas) = self.reference.as_canvas().filter(CanvasHandle::is_attached) {
      draw(&canvas, &plot);
    }
    *self.plot.lock() = Some(plot);
    Canvas::new()
      .ref_element(self.reference.clone())
      .id(id)
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(CHART_HEIGHT))
  }

  fn after_layout(&self) {
    if let Some(canvas) = self.reference.as_canvas() {
      self.redraw.bind(&canvas, &self.plot, draw);
    }
  }
}

fn draw(canvas: &CanvasHandle, plot: &Plot) {
  let context = canvas.context_2d();
  context.reset();
  let frame = Frame::of(canvas, 0.0);
  let bars = &plot.props.bars;
  let largest = bars
    .iter()
    .map(|bar| bar.value)
    .chain(plot.props.reference)
    .fold(0.0_f64, f64::max);
  let (top, step) = plot::scale(largest.ceil() as u64);
  plot::grid(&context, frame, &plot.style, (top, step), |value| value.to_string());
  let mut items = Vec::new();
  context.set_fill_style(plot.style.series[0]);
  for (index, bar) in bars.iter().enumerate() {
    let (slot, width) = frame.slot(index, bars.len());
    let wide = (width * BAR_SHARE).clamp(1.0, BAR_MAX);
    let left = slot + (width - wide) / 2.0;
    let y = frame.y(bar.value, top as f64);
    if bar.value > 0.0 {
      context.fill_rect(left, y, wide, frame.bottom - y);
    }
    items.push(
      CanvasItem::rect(format!("bar-{index}"), "bar", left, y, wide, frame.bottom - y)
        .label(bar.name.as_ref())
        .value(bar.text.as_ref()),
    );
    if let Some(name) = &bar.label {
      let label = XLabel {
        name,
        kind: AxisText::Day,
        x: slot + width / 2.0,
      };
      items.extend(plot::x_label(&context, frame, &plot.style, &label, index));
    }
  }
  if let Some(reference) = plot.props.reference {
    let y = frame.y(reference, top as f64);
    context.set_line_width(LINE_WIDTH);
    context.set_stroke_style(plot.style.single);
    context.set_line_dash(&DASH);
    context.begin_path();
    context.move_to(frame.left, y);
    context.line_to(frame.right, y);
    context.stroke();
    context.set_line_dash(&[]);
  }
  canvas.set_items(items);
}
