//! Clickable surfaces. Outside forms (navigation, tabs, the title bar, list rows, toolbars, the status
//! bar) they are pointer-only: a press never moves the focus and Tab skips them. A form control (a
//! form's or a card's buttons, a switch, a checkbox, a segmented control) is a Tab stop that Enter or
//! Space presses, and shows the focus ring while focused (`docs/design/kontur-handover.md` §5.4, §5.5).

use std::sync::Arc;

use lurq::{
  app::events::mouse_event::MouseEvent,
  components::Button,
  layout::Alignment,
  node::{CursorIcon, Style},
};

use crate::{
  navigation::link::{Open, keeps_middle_click, navigates},
  theme::{ExtraRadius, ExtraShadow},
};

/// lurq's tab index for a control Tab reaches in the order the window shows it.
const TAB_STOP: i32 = 0;

/// An empty button laid out like a `Row`, pressed with the pointer only, running `on_click`. The press
/// is its own: lurq hands a click to every ancestor, and an openable row around it must not open too.
pub fn pressable(on_click: impl Fn() + Send + Sync + 'static) -> Button {
  Press::run(on_click).on(surface())
}

/// [`pressable`] for a control that navigates: it opens its place with `open` by a browser's rules
/// (`navigation::link`).
pub fn opening(open: Open) -> Button {
  Press::Open(open).on(surface())
}

fn surface() -> Button {
  Button::empty()
    .align_items(Alignment::Center)
    .focusable(false)
    .cursor(CursorIcon::Pointer)
}

/// What a press on a control does: run an action, or open a place by a browser's rules (`navigation::link`).
#[derive(Clone)]
pub enum Press {
  Run(Arc<dyn Fn() + Send + Sync>),
  Open(Open),
}

impl Press {
  pub fn run(action: impl Fn() + Send + Sync + 'static) -> Self {
    Self::Run(Arc::new(action))
  }

  /// `button` pressed this way. The press is its own: it does not also press a clickable row around it, with the
  /// left button or the middle one.
  pub fn on(self, button: Button) -> Button {
    match self {
      Self::Run(action) => keeps_middle_click(button).on_click(move |event: MouseEvent| {
        event.stop_propagation();
        action();
      }),
      Self::Open(open) => navigates(button, open),
    }
  }

  /// [`pressable`] or [`opening`], as this press is.
  pub fn button(self) -> Button {
    self.on(surface())
  }
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
  /// A control the keyboard reaches too, a form's or any other: a Tab stop that Enter or Space presses, with the
  /// focus ring inside its edge only while the keyboard moved the focus there, never after a press or when the
  /// page focuses it by itself.
  Keyboard,
  /// A form control that is only words, with no padding of its own (a link, a disclosure line): a Tab stop that
  /// Enter or Space presses, with the link's ring outside its words, corners rounded, only while the keyboard moved
  /// the focus there (`Mjhop`). An inset ring would run through its words. lurq clips a child at its parent's edge,
  /// so every parent between the control and one with room around it is `overflow_visible`: nothing moves, only
  /// the clip goes.
  Link,
}

impl Focus {
  /// `button`, which can be pressed, as this kind of control.
  pub fn apply(self, button: Button) -> Button {
    match self {
      Self::Pointer => button.focusable(false),
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
  keeps_middle_click(button)
    .focusable(false)
    .cursor(CursorIcon::NotAllowed)
    .on_click(|event: MouseEvent| event.stop_propagation())
}
