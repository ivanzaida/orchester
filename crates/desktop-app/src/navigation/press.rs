//! How a press on a link asks to open its place, by a browser's rules: a plain click opens it in the active tab;
//! Ctrl+click (Cmd+click on macOS) and a middle click in a new tab right of the active one, which stays active;
//! Ctrl+Shift+click (Cmd+Shift+click) in a new tab it switches to. Shift alone opens no window here, so it is a
//! plain click. Enter on a focused link is a click too, with the keys held. lurq hands the app the event that
//! activated a Markdown link (`MarkdownProps::on_link_activate`), its button, modifiers and source included.

use lurq::app::events::{MouseButton, MouseEvent};

/// Where a place a link names opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenIn {
  /// The active tab goes there.
  ActiveTab,
  /// A new tab right of the active one, which stays active.
  BackgroundTab,
  /// A new tab right of the active one, which becomes active.
  NewTab,
}

/// A press on a link: its button, and the modifiers that decide where it opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkPress {
  pub button: MouseButton,
  pub shift: bool,
  /// The platform's command modifier: Ctrl, Cmd on macOS.
  pub command: bool,
}

impl LinkPress {
  /// A plain click.
  pub const PLAIN: Self = Self {
    button: MouseButton::Left,
    shift: false,
    command: false,
  };

  /// The press `event` made on a link.
  pub const fn of(event: &MouseEvent) -> Self {
    Self {
      button: event.button,
      shift: event.shift,
      command: platform::command(event),
    }
  }

  /// Where the press opens its place; `None` for a button that opens nothing.
  pub const fn open_in(self) -> Option<OpenIn> {
    match self.button {
      MouseButton::Middle => Some(OpenIn::BackgroundTab),
      MouseButton::Left => Some(match (self.command, self.shift) {
        (true, true) => OpenIn::NewTab,
        (true, false) => OpenIn::BackgroundTab,
        (false, _) => OpenIn::ActiveTab,
      }),
      MouseButton::Right | MouseButton::Other(_) => None,
    }
  }
}

#[cfg(not(target_os = "macos"))]
mod platform {
  use lurq::app::events::MouseEvent;

  /// Ctrl.
  pub(super) const fn command(event: &MouseEvent) -> bool {
    event.ctrl
  }
}

#[cfg(target_os = "macos")]
mod platform {
  use lurq::app::events::MouseEvent;

  /// Cmd: Ctrl+click is the secondary click there.
  pub(super) const fn command(event: &MouseEvent) -> bool {
    event.meta
  }
}
