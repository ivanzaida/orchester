//! Clickable surfaces. Outside forms (navigation, tabs, the title bar, list rows, toolbars, the status
//! bar) they are pointer-only: a press never moves the focus and Tab skips them. A form control (a
//! form's or a card's buttons, a switch, a checkbox, a segmented control) is a Tab stop that Enter or
//! Space presses, and shows the focus ring while focused (`docs/design/kontur-handover.md` §5.4, §5.5).

use lurq::{
  components::Button,
  layout::Alignment,
  node::{CursorIcon, Style},
};

use crate::theme::ExtraShadow;

/// lurq's tab index for a control Tab reaches in the order the window shows it.
const TAB_STOP: i32 = 0;

/// An empty button laid out like a `Row`, pressed with the pointer only, running `on_click`.
pub fn pressable(on_click: impl Fn() + Send + Sync + 'static) -> Button {
  Button::empty()
    .align_items(Alignment::Center)
    .focusable(false)
    .cursor(CursorIcon::Pointer)
    .on_click(move |_| on_click())
}

/// How a control takes a press.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Focus {
  /// With the pointer only: never a Tab stop, no focus ring.
  #[default]
  Pointer,
  /// A form control: a Tab stop that Enter or Space presses, with the focus ring while focused. The
  /// ring is a shadow outside the control, so its parents must leave it room.
  Form,
}

impl Focus {
  /// `button`, which can be pressed, as this kind of control.
  pub fn apply(self, button: Button) -> Button {
    match self {
      Self::Pointer => button.focusable(false),
      Self::Form => button
        .button()
        .focusable(true)
        .tab_index(TAB_STOP)
        .focused_style(Style::new().box_shadow(ExtraShadow::FocusRing.role())),
    }
  }
}

/// `button`, which cannot be pressed now: never a Tab stop, and the pointer says so.
pub fn inert(button: Button) -> Button {
  button.focusable(false).cursor(CursorIcon::NotAllowed)
}
