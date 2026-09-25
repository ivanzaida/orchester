//! Clickable controls that are not text fields. Only form controls take focus: a button in a
//! form (dialog, new task, setup, inspector and editor fields) is a lurq button, so Enter and
//! Space press it, and a Tab stop with `tab_index(0)`, since lurq puts elements outside a lurq
//! form in the Tab order only on request. Everything outside a form (navigation, tabs, list
//! rows, canvas cards, toolbars, page actions, the status bar) is pointer-only: `focusable(false)`,
//! so it is no Tab stop, shows no ring, and a press on it leaves the focus where it is.
//!
//! A focused control shows the focus ring (`xURvJ`): box shadows only, so it never moves
//! layout. Hover and press styles must not set a shadow, or they would hide the ring. Fields and
//! selects keep their focused stroke (`o8Oug`) instead.

use lurq::{
  components::Button,
  layout::Alignment,
  node::{CursorIcon, Style},
};

use crate::theme::ExtraShadow;

/// lurq's tab index for a control that Tab reaches in the order the window shows it.
pub const TAB_STOP: i32 = 0;

/// Where the focus ring of a form control is drawn.
#[derive(Clone, Copy)]
pub enum Ring {
  /// Around the control, 4px out (buttons, option cards). Needs unclipped room.
  Outer,
  /// On the control's edge (rows stacked edge to edge, toggles in a tight row).
  Inset,
}

impl Ring {
  /// The focused style that draws this ring.
  pub fn focused(self) -> Style {
    Style::new().box_shadow(match self {
      Self::Outer => ExtraShadow::FocusRing.role(),
      Self::Inset => ExtraShadow::FocusRingInset.role(),
    })
  }
}

/// An empty button laid out like a `Row` (children start at the top), for a clickable surface.
/// It never takes focus unless [`keyboard_stop`] makes a form control of it.
pub fn surface() -> Button {
  Button::empty().button().align_items(Alignment::Start).focusable(false)
}

/// Makes `button` press `on_click` with the pointer only, for a control outside a form.
pub fn pressable(button: Button, on_click: impl Fn() + Send + Sync + 'static) -> Button {
  button
    .focusable(false)
    .cursor(CursorIcon::Pointer)
    .on_click(move |_| on_click())
}

/// A form control: a Tab stop showing `ring` while focused, for a button that has its click
/// handler.
pub fn keyboard_stop(button: Button, ring: Ring) -> Button {
  button.focusable(true).tab_index(TAB_STOP).focused_style(ring.focused())
}
