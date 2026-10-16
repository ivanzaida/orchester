//! Clickable surfaces. Outside forms (navigation, tabs, the title bar, list rows, toolbars, the status
//! bar) they are pointer-only: a press never moves the focus and Tab skips them. A form control (a
//! form's or a card's buttons, a switch, a checkbox, a segmented control) is a Tab stop that Enter or
//! Space presses, and shows the focus ring while focused (`docs/design/kontur-handover.md` §5.4, §5.5).

use lurq::{
  app::events::mouse_event::MouseEvent,
  components::Button,
  layout::Alignment,
  node::{CursorIcon, Style},
};

use crate::theme::{ExtraRadius, ExtraShadow};

/// lurq's tab index for a control Tab reaches in the order the window shows it.
const TAB_STOP: i32 = 0;

/// An empty button laid out like a `Row`, pressed with the pointer only, running `on_click`. The press
/// is its own: lurq hands a click to every ancestor, and an openable row around it must not open too.
pub fn pressable(on_click: impl Fn() + Send + Sync + 'static) -> Button {
  Button::empty()
    .align_items(Alignment::Center)
    .focusable(false)
    .cursor(CursorIcon::Pointer)
    .on_click(move |event: MouseEvent| {
      event.stop_propagation();
      on_click();
    })
}

/// `button`, a control in the custom title bar. The bar hands a press that reaches it to the OS to move
/// the window, and the OS move loop takes the release, so the control would never be clicked: its press
/// stops here, and only the bar's empty room moves the window.
pub fn in_title_bar(button: Button) -> Button {
  button.on_mouse_down(|event: MouseEvent| event.stop_propagation())
}

/// How a control takes a press.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Focus {
  /// With the pointer only: never a Tab stop, no focus ring.
  #[default]
  Pointer,
  /// A form control: a Tab stop that Enter or Space presses, with the focus ring inside its edge while
  /// focused. lurq 0.42 has no
  /// keyboard-only focus state, so a pointer press that focuses the control shows the ring too.
  Form,
  /// A control outside forms the keyboard reaches too: a Tab stop that Enter or Space presses, with the
  /// focus ring only while the keyboard moved the focus there, never after a press.
  Keyboard,
  /// A form control that is only words, with no padding of its own (a link, a disclosure line): a Tab stop that
  /// Enter or Space presses, with the link's ring outside its words, corners rounded, only while the keyboard moved
  /// the focus there (`Mjhop`). An inset ring would run through its words.
  Link,
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
      Self::Keyboard => button
        .button()
        .focusable(true)
        .tab_index(TAB_STOP)
        .focus_visible_style(Style::new().box_shadow(ExtraShadow::FocusRing.role())),
      Self::Link => button.button().focusable(true).tab_index(TAB_STOP).focus_visible_style(
        Style::new()
          .box_shadow(ExtraShadow::LinkFocusRing.role())
          .rounded(ExtraRadius::Link.role()),
      ),
    }
  }
}

/// `button`, which cannot be pressed now: never a Tab stop, and the pointer says so. A press on it does
/// nothing, and does not reach an openable row around it either.
pub fn inert(button: Button) -> Button {
  button
    .focusable(false)
    .cursor(CursorIcon::NotAllowed)
    .on_click(|event: MouseEvent| event.stop_propagation())
}
