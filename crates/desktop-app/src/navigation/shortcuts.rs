//! What a key press asks of the window: Back, Forward and the tabs. Implemented so far: Ctrl+Tab and
//! Ctrl+Shift+Tab switch tabs on every platform; Ctrl+T / Ctrl+W and Alt+Left / Alt+Right on Windows,
//! Cmd+T / Cmd+W and Cmd+[ / Cmd+] on macOS. Alt+Left and Alt+Right are left to a focused text field,
//! which moves by words with them. The rest of the design's table (`docs/design/orchester-workspace.md`,
//! "Tabs" › "Keyboard") is not built yet.
//!
//! A shortcut is the key as typed: on AZERTY, Ctrl+Z (typed at the place of QWERTY's W) is not Ctrl+W,
//! and on Dvorak Ctrl+, (typed there too) is not either. Only where the layout types no ASCII character
//! (Cyrillic, Greek…) the key's place stands in for it.

use lurq::app::events::KeyboardEvent;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shortcut {
  Back,
  Forward,
  NewTab,
  CloseTab,
  NextTab,
  PreviousTab,
}

/// The parts of a key press that decide the shortcut.
#[derive(Clone, Copy, Debug, Default)]
pub struct Keys<'a> {
  /// The key as typed (`"t"`, `"ArrowLeft"`).
  pub key: &'a str,
  /// The key's place on the keyboard (`"KeyT"`), for layouts that type something else there.
  pub code: &'a str,
  pub ctrl: bool,
  pub alt: bool,
  pub meta: bool,
  pub shift: bool,
  /// Whether a text field has the keyboard focus.
  pub typing: bool,
}

impl<'a> Keys<'a> {
  pub fn of(event: &'a KeyboardEvent) -> Self {
    Self {
      key: &event.key,
      code: &event.code,
      ctrl: event.ctrl,
      alt: event.alt,
      meta: event.meta,
      shift: event.shift,
      typing: event.text_input_focused,
    }
  }

  /// Whether this is the key `key` as typed; or, when the layout types no ASCII character here, the key
  /// at `code`.
  pub fn is(self, key: &str, code: &str) -> bool {
    self.key.eq_ignore_ascii_case(key) || (!self.key.is_ascii() && self.code == code)
  }

  /// Only `ctrl` of the modifiers other than Shift.
  const fn only_ctrl(self) -> bool {
    self.ctrl && !self.alt && !self.meta
  }

  /// Only the platform's command modifier, Ctrl (Cmd on macOS), and no Shift: Ctrl+F, Cmd+F.
  pub const fn only_command(self) -> bool {
    !self.shift && platform::command_only(self)
  }
}

/// The shortcut `keys` are on this platform, if any.
pub fn shortcut(keys: Keys<'_>) -> Option<Shortcut> {
  if keys.only_ctrl() && keys.is("Tab", "Tab") {
    return Some(if keys.shift {
      Shortcut::PreviousTab
    } else {
      Shortcut::NextTab
    });
  }
  platform::shortcut(keys)
}

/// Whether `keys` open the title bar's tab list: Ctrl+Shift+A, Cmd+Shift+A on macOS (`YAOSy`). The list
/// takes them while it is offered; they are no window shortcut otherwise.
pub fn opens_tab_list(keys: Keys<'_>) -> bool {
  keys.shift && platform::command_only(keys) && keys.is("a", "KeyA")
}

#[cfg(not(target_os = "macos"))]
mod platform {
  use super::{Keys, Shortcut};

  /// Ctrl and no other modifier but Shift.
  pub(super) const fn command_only(keys: Keys<'_>) -> bool {
    keys.only_ctrl()
  }

  pub(super) fn shortcut(keys: Keys<'_>) -> Option<Shortcut> {
    if keys.only_ctrl() && !keys.shift {
      if keys.is("t", "KeyT") {
        return Some(Shortcut::NewTab);
      }
      if keys.is("w", "KeyW") {
        return Some(Shortcut::CloseTab);
      }
    }
    if keys.alt && !keys.ctrl && !keys.meta && !keys.shift && !keys.typing {
      if keys.is("ArrowLeft", "ArrowLeft") {
        return Some(Shortcut::Back);
      }
      if keys.is("ArrowRight", "ArrowRight") {
        return Some(Shortcut::Forward);
      }
    }
    None
  }
}

#[cfg(target_os = "macos")]
mod platform {
  use super::{Keys, Shortcut};

  /// Cmd and no other modifier but Shift.
  pub(super) const fn command_only(keys: Keys<'_>) -> bool {
    keys.meta && !keys.ctrl && !keys.alt
  }

  pub(super) fn shortcut(keys: Keys<'_>) -> Option<Shortcut> {
    if !keys.meta || keys.ctrl || keys.alt || keys.shift {
      return None;
    }
    [
      ("t", "KeyT", Shortcut::NewTab),
      ("w", "KeyW", Shortcut::CloseTab),
      ("[", "BracketLeft", Shortcut::Back),
      ("]", "BracketRight", Shortcut::Forward),
    ]
    .into_iter()
    .find(|(key, code, _)| keys.is(key, code))
    .map(|(_, _, shortcut)| shortcut)
  }
}
