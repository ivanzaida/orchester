//! Links: controls that open a place — a page of a project, All projects, a project — the way the
//! press asks (design doc, "Tabs" › "Opening and closing"). A plain click opens it in the current
//! tab; Ctrl/Cmd+click and a middle click in a new tab behind the current one; Ctrl/Cmd+Shift+click
//! in a new tab that becomes active. Enter and Space on a focused form control that is a link open
//! the place in the current tab. Every link reads the press through [`Link`], so they all agree;
//! navigation that is not a link (a submitted form, a dialog's result, a redirect after an action)
//! stays in the current tab and does not use it.

use std::sync::Arc;

use lurq::{
  app::events::{MouseButton, MouseEvent},
  components::Button,
  node::CursorIcon,
};

use super::pressable::pressable;
use crate::tabs::{OpenIn, Tabs};

type Open = Arc<dyn Fn(OpenIn) + Send + Sync>;
type Action = Arc<dyn Fn() + Send + Sync>;

/// A place a press opens, where the press asks.
#[derive(Clone)]
pub struct Link {
  tabs: Tabs,
  open: Open,
}

impl Link {
  /// `open` opens the place where it is asked to.
  pub fn new(tabs: &Tabs, open: impl Fn(OpenIn) + Send + Sync + 'static) -> Self {
    Self {
      tabs: tabs.clone(),
      open: Arc::new(open),
    }
  }

  /// Makes `button` this link, pointer-only like any control outside a form (see
  /// `ui::pressable`).
  pub fn on(self, button: Button) -> Button {
    self.attach(button.focusable(false))
  }

  /// Adds this link to `button`, which keeps its focus: a form control stays a Tab stop, and
  /// Enter or Space on it opens the place in the current tab.
  pub fn attach(self, button: Button) -> Button {
    let (pressed, click, middle) = (self.tabs.clone(), self.clone(), self.open);
    button
      .cursor(CursorIcon::Pointer)
      .on_mouse_down(move |_: MouseEvent| pressed.pointer_pressed())
      .on_click(move |event: MouseEvent| (click.open)(click.tabs.open_in(&event)))
      .on_mouse_click(MouseButton::Middle, move |_: MouseEvent| middle(OpenIn::Background))
  }
}

/// What pressing a control does: an action, which the press runs whatever keys are held, or a
/// link, which opens its place where the press asks.
#[derive(Clone)]
pub enum Press {
  Action(Action),
  Link(Link),
}

impl Press {
  pub fn action(action: impl Fn() + Send + Sync + 'static) -> Self {
    Self::Action(Arc::new(action))
  }

  /// Makes `button` do this on a pointer press only (see `ui::pressable`).
  pub fn on(self, button: Button) -> Button {
    match self {
      Self::Action(action) => pressable(button, move || action()),
      Self::Link(link) => link.on(button),
    }
  }

  /// Adds this to `button`, a form control that keeps its focus: Enter and Space press it too.
  pub fn attach(self, button: Button) -> Button {
    match self {
      Self::Action(action) => button
        .cursor(CursorIcon::Pointer)
        .on_click(move |_: MouseEvent| action()),
      Self::Link(link) => link.attach(button),
    }
  }
}
