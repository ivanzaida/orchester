//! Keyboard use of everything clickable that is not a form field: navigation items, tabs, list
//! rows, toolbar and dialog buttons, status bar items. Each is a lurq button, so Enter and
//! Space press it, and a Tab stop with `tab_index(0)`, since lurq puts elements outside a form
//! in the Tab order only on request.
//!
//! A focused control shows the focus ring (`xURvJ`): box shadows only, so it never moves
//! layout. Hover and press styles must not set a shadow, or they would hide the ring. Fields and
//! selects keep their focused stroke (`o8Oug`) instead. A control whose selected state is the
//! selection stroke hides the ring while selected (see [`stroke_selection_ring`]).

use lurq::{
  components::Button,
  layout::Alignment,
  node::{CursorIcon, Style, border::Borders},
};

use crate::theme::ExtraShadow;

/// lurq's tab index for a control that Tab reaches in the order the window shows it.
pub const TAB_STOP: i32 = 0;

/// Where the focus ring is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ring {
  /// Around the control, 4px out (buttons, navigation items, cards). Needs unclipped room.
  Outer,
  /// On the control's edge (tabs, rows stacked edge to edge, popover rows, status bar items).
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

/// Focus style of a row stacked on others with a bottom hairline: the inset ring covers the
/// whole row as in the design (`X9gPDr`). lurq casts inset shadows inside an inside border and
/// paints the border over them, so while focused the row drops its border; borders take no
/// layout space, so nothing moves.
pub fn hairline_row_focused() -> Style {
  Ring::Inset.focused().border_custom(Borders::default())
}

/// The ring of a control whose selected state is the 1.5px selection stroke (step card `ajikA`,
/// new pipeline option card): none while selected, since stroke, gap and ring read as a double
/// border and the stroke already marks the control; `ring` otherwise.
pub fn stroke_selection_ring(ring: Ring, selected: bool) -> Option<Ring> {
  (!selected).then_some(ring)
}

/// Focus style of a control whose selection is a stroke; see [`stroke_selection_ring`]. Without a
/// ring, focus leaves the control's look unchanged.
pub fn stroke_selected_focused(ring: Ring, selected: bool) -> Style {
  stroke_selection_ring(ring, selected).map_or_else(Style::new, Ring::focused)
}

/// An empty button laid out like a `Row` (children start at the top), for a clickable surface.
/// It takes no focus until [`pressable`] makes it act.
pub fn surface() -> Button {
  Button::empty().button().align_items(Alignment::Start).focusable(false)
}

/// Makes `button` press `on_click`: pointer, Tab stop, focus ring.
pub fn pressable(button: Button, ring: Ring, on_click: impl Fn() + Send + Sync + 'static) -> Button {
  keyboard_stop(button, ring)
    .cursor(CursorIcon::Pointer)
    .on_click(move |_| on_click())
}

/// A Tab stop showing `ring` while focused, for a button that already has its click handler.
pub fn keyboard_stop(button: Button, ring: Ring) -> Button {
  button.focusable(true).tab_index(TAB_STOP).focused_style(ring.focused())
}
