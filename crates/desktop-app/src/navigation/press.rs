//! How a press on a link asks to open its place, by a browser's rules: a plain click opens it in the active tab;
//! Ctrl+click (Cmd+click on macOS) and a middle click in a new tab right of the active one, which stays active;
//! Ctrl+Shift+click (Cmd+Shift+click) in a new tab it switches to. Shift alone opens no window here, so it is a
//! plain click, and so is Enter on a focused link. The right and the side buttons open nothing.
//!
//! lurq hands a Markdown link's press to the app without its button or keys, and its mouse events carry Ctrl, Shift
//! and Alt but not Cmd. So the window records the pointer's last press and follows Cmd's presses and releases
//! ([`PressInput`], fed by `input::window_input`), and a link reads them when its press arrives
//! ([`Navigation::link_press`](super::Navigation::link_press)). Synthetic input reaches the window the same way.

use lurq::app::events::{KeyboardEvent, MouseButton, MouseEvent};

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
  /// A plain click, as Enter on a focused link is too.
  pub const PLAIN: Self = Self {
    button: MouseButton::Left,
    shift: false,
    command: false,
  };

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

/// What the window last saw of the pointer and the keys.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct PressInput {
  /// The pointer's last press: its button, and whether Shift and Ctrl were held. Enter going down forgets it: lurq
  /// presses a focused link for Enter with a click of its own, after the key and without a pointer press.
  pointer: Option<(MouseButton, bool, bool)>,
  /// Whether Cmd is held, as the keys last said.
  meta: bool,
}

impl PressInput {
  pub(super) fn pointer_down(&mut self, event: &MouseEvent) {
    self.pointer = Some((event.button, event.shift, event.ctrl));
  }

  /// A key went down (`down`) or up.
  pub(super) fn key(&mut self, event: &KeyboardEvent, down: bool) {
    self.meta = if matches!(event.key.as_str(), "Meta" | "Super") {
      down
    } else {
      event.meta
    };
    if down && (event.key == "Enter" || event.code == "Enter") {
      self.pointer = None;
    }
  }

  /// The press a link that is pressed now was given.
  pub(super) fn press(self) -> LinkPress {
    let Some((button, shift, ctrl)) = self.pointer else {
      return LinkPress::PLAIN;
    };
    LinkPress {
      button,
      shift,
      command: platform::command(ctrl, self.meta),
    }
  }
}

#[cfg(not(target_os = "macos"))]
mod platform {
  /// Ctrl.
  pub(super) const fn command(ctrl: bool, _meta: bool) -> bool {
    ctrl
  }
}

#[cfg(target_os = "macos")]
mod platform {
  /// Cmd: Ctrl+click is the secondary click there.
  pub(super) const fn command(_ctrl: bool, meta: bool) -> bool {
    meta
  }
}
