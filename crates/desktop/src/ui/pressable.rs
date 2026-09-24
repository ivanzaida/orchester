//! Keyboard use of everything clickable that is not a form field: navigation items, tabs, list
//! rows, toolbar and dialog buttons, status bar items. Each is a lurq button, so Enter and
//! Space press it, and a Tab stop with `tab_index(0)`, since lurq puts elements outside a form
//! in the Tab order only on request. The design draws no focus mark for buttons or rows; they
//! take the focused field stroke (`o8Oug`), its only focus mark.

use lurq::{
  app::theme::{BorderSize, PaletteColor},
  components::Button,
  layout::Alignment,
  node::{CursorIcon, Style, border::Border},
};

/// lurq's tab index for a control that Tab reaches in the order the window shows it.
pub const TAB_STOP: i32 = 0;

/// The focused field stroke (`o8Oug`): an inside stroke, so it paints without moving layout.
pub fn focus_stroke() -> Style {
  Style::new().border(Border::inside(BorderSize::Md, PaletteColor::BorderFocus))
}

/// An empty button laid out like a `Row` (children start at the top), for a clickable surface.
/// It takes no focus until [`pressable`] makes it act.
pub fn surface() -> Button {
  Button::empty().button().align_items(Alignment::Start).focusable(false)
}

/// Makes `button` press `on_click`: pointer, Tab stop, focused stroke.
pub fn pressable(button: Button, on_click: impl Fn() + Send + Sync + 'static) -> Button {
  keyboard_stop(button)
    .cursor(CursorIcon::Pointer)
    .on_click(move |_| on_click())
}

/// A Tab stop marked while focused, for a button that already has its click handler.
pub fn keyboard_stop(button: Button) -> Button {
  button.focusable(true).tab_index(TAB_STOP).focused_style(focus_stroke())
}
