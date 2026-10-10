//! The macOS window controls (`Traffic lights` `R3RtGs` in `V70DN`): close, minimize and zoom, in lurq's
//! traffic-light colors, without glyphs, as lurq draws them. lurq's own close light closes the window
//! outright after its handler, which the window's close flow could not hold; these ask to close instead
//! (`lifecycle`), as the window's close request from the system does. A component of their own, so a
//! change of the window (zoomed or not) renders them alone.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::MouseEvent,
    theme::SpacingSize,
  },
  components::{Row, TrafficLightColors},
  layout::{Alignment, layout_kind::Justify},
  node::{BackgroundColor, CursorIcon, Element, Style, dimension::Dimension, padding::Padding},
};

use super::Close;
use crate::theme::ExtraRadius;

/// The room of one light, and the light (lurq's `macos_control_button`).
const SLOT_WIDTH: f32 = 20.0;
const LIGHT_SIZE: f32 = 12.0;

/// What the close light does.
#[derive(Clone)]
pub struct LightsProps(pub Close);

impl PartialEq for LightsProps {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl DevtoolsInspectable for LightsProps {}

/// Close, minimize and zoom.
pub struct TrafficLights;

impl Component for TrafficLights {
  type Props = LightsProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let close = ctx.props::<LightsProps>().0.clone();
    let colors = TrafficLightColors::default();
    let window = ctx.window();
    // lurq 0.42 tells a component the window's state as of its render, and renders it again when the
    // state changes, so this is the state the person sees when pressing zoom.
    let maximized = window.info().is_maximized;
    let (minimize, zoom) = (window.clone(), window);
    let name = |key: &str| ctx.t(key);
    Row::new()
      .id("window-traffic-lights")
      .height(Dimension::Pct(100.0))
      .padding(Padding::horizontal(SpacingSize::Sm))
      .align_items(Alignment::Center)
      .child(light(
        ("window-close", name("shell.window.close")),
        (colors.close, colors.close_active),
        move || close(),
      ))
      .child(light(
        ("window-minimize", name("shell.window.minimize")),
        (colors.minimize, colors.minimize_active),
        move || minimize.set_minimized(true),
      ))
      .child(light(
        (
          "window-zoom",
          name(if maximized {
            "shell.window.restore"
          } else {
            "shell.window.maximize"
          }),
        ),
        (colors.zoom, colors.zoom_active),
        move || zoom.set_maximized(!maximized),
      ))
  }
}

fn light(
  (id, name): (&str, Arc<str>),
  (color, active): (BackgroundColor, BackgroundColor),
  press: impl Fn() + Send + Sync + 'static,
) -> Row {
  Row::new()
    .id(id.to_owned())
    .describe("a11y_name", name)
    .width(Dimension::Px(SLOT_WIDTH))
    .height(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .cursor(CursorIcon::Pointer)
    // The title bar drags the window from a press; a light is not part of the drag area.
    .on_mouse_down(|event: MouseEvent| {
      event.prevent_default();
      event.stop_immediate_propagation();
    })
    .on_click(move |event: MouseEvent| {
      press();
      event.prevent_default();
      event.stop_immediate_propagation();
    })
    .child(
      Row::new()
        .width(Dimension::Px(LIGHT_SIZE))
        .height(Dimension::Px(LIGHT_SIZE))
        .rounded(ExtraRadius::Full.role())
        .background(color.clone())
        .hovered_style(Style::new().background(color))
        .active_style(Style::new().background(active)),
    )
}
