//! The macOS window controls (`Traffic lights` `R3RtGs` in `V70DN`): close, minimize and zoom, in lurq's
//! traffic-light colors. lurq's own close light closes the window outright after its handler, which the
//! window's close flow could not hold; these ask to close instead (`lifecycle`), as the window's close
//! request from the system does.

use lurq::{
  app::{ctx::Ctx, events::MouseEvent},
  components::{Row, TrafficLightColors},
  layout::{Alignment, layout_kind::Justify},
  node::{BackgroundColor, CursorIcon, Element, Style, dimension::Dimension, padding::Padding},
};

use super::Close;
use crate::theme::ExtraRadius;

/// The room of one light, and the light (lurq's `macos_control_button`).
const SLOT_WIDTH: f32 = 20.0;
const LIGHT_SIZE: f32 = 12.0;

/// Close, minimize and zoom.
pub fn lights(ctx: &mut Ctx, close: Close) -> Element {
  let colors = TrafficLightColors::default();
  let window = ctx.window();
  let (minimize, zoom) = (window.clone(), window.clone());
  let maximized = window.info().is_maximized;
  Row::new()
    .id("window-traffic-lights")
    .height(Dimension::Pct(100.0))
    .padding(Padding::horizontal(lurq::app::theme::SpacingSize::Sm))
    .align_items(Alignment::Center)
    .child(light("window-close", (colors.close, colors.close_active), move || {
      close()
    }))
    .child(light(
      "window-minimize",
      (colors.minimize, colors.minimize_active),
      move || minimize.set_minimized(true),
    ))
    .child(light("window-zoom", (colors.zoom, colors.zoom_active), move || {
      zoom.set_maximized(!maximized);
    }))
    .into()
}

fn light(
  id: &str,
  (color, active): (BackgroundColor, BackgroundColor),
  press: impl Fn() + Send + Sync + 'static,
) -> Row {
  Row::new()
    .id(id.to_owned())
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
