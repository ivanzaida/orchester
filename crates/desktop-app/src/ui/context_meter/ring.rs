//! The meter's ring (`Ring` `sXSPk`): a 12 annulus, its inner edge at 60 % of its radius, the track in
//! `border_strong` and the used share over it from 12 o'clock clockwise, in the state's color.

use std::{
  f32::consts::{FRAC_PI_2, TAU},
  sync::Arc,
};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::PaletteColor,
  },
  canvas::{ArcDirection, CanvasHandle},
  components::Canvas,
  core::ElementRef,
  node::{Element, color::Color, dimension::Dimension},
};
use parking_lot::Mutex;

use crate::screens::usage::plot::Redraw;

/// The ring's side (`sXSPk`), and its inner radius as a share of its outer one (`innerRadius` 0.6).
pub(super) const RING_SIZE: f32 = 12.0;
const INNER: f32 = 0.6;

/// What the ring shows: the share used, in percent, and its color.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct RingProps {
  pub(super) id: Arc<str>,
  pub(super) share: u64,
  pub(super) color: PaletteColor,
}

impl DevtoolsInspectable for RingProps {}

/// The colors and share the canvas draws.
#[derive(Clone, Copy)]
struct Drawn {
  share: u64,
  track: Color,
  used: Color,
}

pub(super) struct Ring {
  reference: ElementRef,
  drawn: Arc<Mutex<Option<Drawn>>>,
  redraw: Redraw,
}

impl Component for Ring {
  type Props = RingProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      reference: ctx.element_ref(),
      drawn: Arc::new(Mutex::new(None)),
      redraw: Redraw::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RingProps>().clone();
    let theme = ctx.theme();
    let drawn = Drawn {
      share: props.share,
      track: theme.palette_color(PaletteColor::BorderStrong),
      used: theme.palette_color(props.color),
    };
    if let Some(canvas) = self.reference.as_canvas().filter(CanvasHandle::is_attached) {
      draw(&canvas, &drawn);
    }
    *self.drawn.lock() = Some(drawn);
    Canvas::new()
      .ref_element(self.reference.clone())
      .id(props.id.to_string())
      .describe("share", props.share.to_string())
      .size(Dimension::Px(RING_SIZE), Dimension::Px(RING_SIZE))
      .flex_shrink(0.0)
  }

  fn after_layout(&self) {
    if let Some(canvas) = self.reference.as_canvas() {
      self.redraw.bind(&canvas, &self.drawn, draw);
    }
  }
}

/// The track as a whole circle, then the used share over it, both stroked along the annulus's middle.
fn draw(canvas: &CanvasHandle, drawn: &Drawn) {
  let context = canvas.context_2d();
  context.reset();
  let outer = RING_SIZE / 2.0;
  let width = outer * (1.0 - INNER);
  let radius = outer - width / 2.0;
  context.set_line_width(width);
  context.set_line_dash(&[]);
  // A share of a whole turn: f32 is precise enough.
  let turn = (drawn.share.min(100) as f32 / 100.0) * TAU;
  let start = -FRAC_PI_2;
  for (color, end) in [(drawn.track, start + TAU), (drawn.used, start + turn)] {
    if end <= start {
      continue;
    }
    context.set_stroke_style(color);
    context.begin_path();
    // An arc of a positive radius is always valid geometry; one the canvas refuses is not drawn.
    if context
      .arc(outer, outer, radius, start, end, ArcDirection::Clockwise)
      .is_ok()
    {
      context.stroke();
    }
  }
}
