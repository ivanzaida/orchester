//! *Cumulative flow* on a canvas, drawn with the Usage page's chart pieces (`screens::usage::plot`): the tasks in
//! each workflow status at the end of each day, stacked from the baseline in [`FLOW_STATUSES`] order (Done at the
//! bottom), each band filled in its series color, with the days on the x axis. Each day is a canvas item naming its
//! counts.

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
use orchester_workspace::delivery::FLOW_STATUSES;
use parking_lot::Mutex;

use crate::{
  screens::usage::plot::{self, AxisName, AxisText, CHART_HEIGHT, Frame, Redraw, XLabel},
  theme::usage::ChartStyle,
};

/// The series color of each band, by [`FLOW_STATUSES`]: Done, In review, In progress, Todo, Backlog.
pub const BAND_SERIES: [usize; FLOW_STATUSES.len()] = [2, 1, 0, 3, 4];

/// One day of the chart.
#[derive(Clone, Debug, PartialEq)]
pub struct FlowPoint {
  pub label: Option<AxisName>,
  pub counts: [u32; 5],
  /// How the day and its counts read, for its canvas item.
  pub name: Arc<str>,
  pub value: Arc<str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlowProps {
  pub days: Arc<[FlowPoint]>,
}

impl DevtoolsInspectable for FlowProps {}

#[derive(Clone)]
struct Plot {
  props: FlowProps,
  style: ChartStyle,
}

pub struct FlowChart {
  reference: ElementRef,
  plot: Arc<Mutex<Option<Plot>>>,
  redraw: Redraw,
}

impl Component for FlowChart {
  type Props = FlowProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      reference: ctx.element_ref(),
      plot: Arc::new(Mutex::new(None)),
      redraw: Redraw::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let plot = Plot {
      props: ctx.props::<FlowProps>().clone(),
      style: ChartStyle::of(ctx),
    };
    if let Some(canvas) = self.reference.as_canvas().filter(CanvasHandle::is_attached) {
      draw(&canvas, &plot);
    }
    *self.plot.lock() = Some(plot);
    Canvas::new()
      .ref_element(self.reference.clone())
      .id("delivery-flow-chart")
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
  let days = &plot.props.days;
  let largest = days
    .iter()
    .map(|day| u64::from(day.counts.iter().sum::<u32>()))
    .max()
    .unwrap_or(0);
  let (top, step) = plot::scale(largest);
  plot::grid(&context, frame, &plot.style, (top, step), |value| value.to_string());
  let center = |index: usize| {
    let (slot, width) = frame.slot(index, days.len());
    slot + width / 2.0
  };
  // Each band from the stack under it to the stack with it, left to right and back.
  let stacked = |index: usize, bands: usize| f64::from(days[index].counts[..bands].iter().sum::<u32>());
  for (band, series) in BAND_SERIES.iter().enumerate().filter(|_| !days.is_empty()) {
    context.set_fill_style(plot.style.series[*series]);
    context.begin_path();
    context.move_to(center(0), frame.y(stacked(0, band + 1), top as f64));
    for index in 1..days.len() {
      context.line_to(center(index), frame.y(stacked(index, band + 1), top as f64));
    }
    for index in (0..days.len()).rev() {
      context.line_to(center(index), frame.y(stacked(index, band), top as f64));
    }
    context.close_path();
    context.fill();
  }
  let mut items = Vec::new();
  for (index, day) in days.iter().enumerate() {
    let (slot, width) = frame.slot(index, days.len());
    items.push(
      CanvasItem::rect(
        format!("flow-day-{index}"),
        "day",
        slot,
        frame.top,
        width,
        frame.height(),
      )
      .label(day.name.as_ref())
      .value(day.value.as_ref()),
    );
    if let Some(name) = &day.label {
      let label = XLabel {
        name,
        kind: AxisText::Day,
        x: center(index),
      };
      items.extend(plot::x_label(&context, frame, &plot.style, &label, index));
    }
  }
  canvas.set_items(items);
}
