//! Clickable surfaces outside forms: navigation, tabs, title bar buttons. They are pointer-only, so a
//! press never moves the focus and Tab skips them.

use lurq::{components::Button, layout::Alignment, node::CursorIcon};

/// An empty button laid out like a `Row`, pressed with the pointer only, running `on_click`.
pub fn pressable(on_click: impl Fn() + Send + Sync + 'static) -> Button {
  Button::empty()
    .align_items(Alignment::Center)
    .focusable(false)
    .cursor(CursorIcon::Pointer)
    .on_click(move |_| on_click())
}
