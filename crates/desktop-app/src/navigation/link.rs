//! Controls that navigate: every one that takes the window to a place (a sidebar entry, a breadcrumb, a row that
//! opens a page, an *Open …* button) opens it by a browser's rules (`press`), as a chat's links do. A plain click
//! and Enter or Space on a focused control open it in the active tab; a middle click and Ctrl+click (Cmd+click on
//! macOS) in a new tab right of the active one, which stays active; Ctrl+Shift+click in a new tab it switches to.
//! A middle press starts no drag of what the control is in. Synthetic input (an MCP client's `lurq_interact`)
//! follows the same rules. A control that acts (Run, Save, a toggle) and only then moves on is not one of these.

use std::sync::Arc;

use lurq::{
  app::events::{MouseButton, MouseEvent},
  components::Button,
};

use super::{
  Navigation,
  press::{LinkPress, OpenIn},
};
use crate::routes::Route;

/// What opens a control's place, where the press asks.
pub type Open = Arc<dyn Fn(OpenIn) + Send + Sync>;

/// `button` opening its place with `open` on a press by a browser's rules. The press is its own: lurq hands a
/// click to every ancestor, and an openable row around the control must not open too.
pub fn navigates(button: Button, open: Open) -> Button {
  let middle = open.clone();
  button
    .on_mouse_down(|event: MouseEvent| hold_middle_press(&event))
    .on_click(move |event: MouseEvent| follow(&open, &event))
    .on_mouse_click(MouseButton::Middle, move |event: MouseEvent| follow(&middle, &event))
}

/// `button`, a control that does not navigate, keeping a middle click to itself: the click does nothing, and does
/// not reach an openable row around the control either, which would open its page in a new tab.
pub fn keeps_middle_click(button: Button) -> Button {
  button
    .on_mouse_down(|event: MouseEvent| hold_middle_press(&event))
    .on_mouse_click(MouseButton::Middle, |event: MouseEvent| event.stop_propagation())
}

fn follow(open: &Open, event: &MouseEvent) {
  event.stop_propagation();
  if let Some(at) = LinkPress::of(event).open_in() {
    open(at);
  }
}

/// A middle press is the start of a middle click, not of a drag of what the control is in.
fn hold_middle_press(event: &MouseEvent) {
  if event.button == MouseButton::Middle {
    event.prevent_default();
  }
}

impl Navigation {
  /// What opens `route` where a press asks (`navigates`).
  pub fn to(&self, route: Route) -> Open {
    let navigation = self.clone();
    Arc::new(move |at| navigation.open(route.clone(), at))
  }
}
