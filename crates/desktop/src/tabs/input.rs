//! The window's input for tabs and history (design doc, "Tabs" › "Keyboard"), handled for the
//! whole window and acting on the active tab. The mouse's side buttons go back and forward on
//! release, as a browser does (the shell reports them as `Other(4)` and `Other(5)`). The key
//! listener is the last node of the tree: keyboard events visit nodes from the root down in order,
//! so every control has seen a key before it and can claim it with `prevent_default`.
//!
//! Tab shortcuts work while no dialog is open; Ctrl/Cmd+W also closes the tab while a text field
//! has the focus. History keeps its shortcuts as before: Alt+Left / Alt+Right (left to a focused
//! text field, which moves by words with them) and Cmd+[ / Cmd+] on macOS.

use lurq::{
  app::events::{KeyboardEvent, MouseButton, MouseEvent},
  components::Column,
  node::{Element, dimension::Dimension},
};

use super::Tabs;

const BACK_BUTTON: MouseButton = MouseButton::Other(4);
const FORWARD_BUTTON: MouseButton = MouseButton::Other(5);
/// Ctrl/Cmd+1 … 8 select tabs 1 … 8; Ctrl/Cmd+9 the last one.
const NUMBERED_TABS: usize = 8;

/// What a key press asks of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shortcut {
  NewTab,
  CloseTab,
  NextTab,
  PreviousTab,
  /// Tab `n`, from 0.
  SelectTab(usize),
  LastTab,
  Back,
  Forward,
}

impl Shortcut {
  /// History shortcuts, which a dialog does not hold back.
  const fn is_history(self) -> bool {
    matches!(self, Self::Back | Self::Forward)
  }
}

/// `content` filling the window, with the tab and history input around it.
pub fn window_input(tabs: &Tabs, content: Element) -> Element {
  let (mouse, keys, released) = (tabs.clone(), tabs.clone(), tabs.clone());
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .on_mouse_up(move |event: MouseEvent| {
      if event.default_prevented() {
        return;
      }
      match event.button {
        BACK_BUTTON => mouse.back(),
        FORWARD_BUTTON => mouse.forward(),
        _ => return,
      }
      event.prevent_default();
    })
    .child(content)
    .child(
      Column::new()
        .on_key_down(move |event: KeyboardEvent| {
          keys.key_pressed(event.meta, activates(&event));
          if event.default_prevented() {
            return;
          }
          let Some(shortcut) = shortcut(&event) else {
            return;
          };
          if !shortcut.is_history() && keys.dialogs().is_open_now() {
            return;
          }
          event.prevent_default();
          run(&keys, shortcut);
        })
        .on_key_up(move |event: KeyboardEvent| released.key_released(event.meta)),
    )
    .into()
}

fn run(tabs: &Tabs, shortcut: Shortcut) {
  match shortcut {
    Shortcut::NewTab => tabs.new_tab(),
    Shortcut::CloseTab => tabs.close_active(),
    Shortcut::NextTab => tabs.next(),
    Shortcut::PreviousTab => tabs.previous(),
    Shortcut::SelectTab(index) => tabs.select(index),
    Shortcut::LastTab => tabs.select_last(),
    Shortcut::Back => tabs.back(),
    Shortcut::Forward => tabs.forward(),
  }
}

/// The shortcut a key press is, on this platform.
pub fn shortcut(event: &KeyboardEvent) -> Option<Shortcut> {
  // Ctrl+Tab switches tabs on every platform.
  if event.ctrl && !event.alt && !event.meta && is(event, "Tab", "Tab") {
    return Some(if event.shift {
      Shortcut::PreviousTab
    } else {
      Shortcut::NextTab
    });
  }
  platform::shortcut(event)
}

/// Whether the key presses a focused control, as lurq does for Enter and Space.
fn activates(event: &KeyboardEvent) -> bool {
  is(event, "Enter", "Enter") || is(event, " ", "Space") || event.code == "NumpadEnter"
}

/// Whether the event is the key `key` (as typed) or `code` (its place on the keyboard).
fn is(event: &KeyboardEvent, key: &str, code: &str) -> bool {
  event.key.eq_ignore_ascii_case(key) || event.code == code
}

/// Tab `n` (from 0) for the digit keys 1…8, the last tab for 9.
fn numbered(event: &KeyboardEvent) -> Option<Shortcut> {
  let digit = event
    .code
    .strip_prefix("Digit")
    .or_else(|| event.code.strip_prefix("Numpad"))
    .unwrap_or(&event.key);
  match digit.parse::<usize>().ok()? {
    9 => Some(Shortcut::LastTab),
    n @ 1..=NUMBERED_TABS => Some(Shortcut::SelectTab(n - 1)),
    _ => None,
  }
}

#[cfg(target_os = "macos")]
mod platform {
  use lurq::app::events::KeyboardEvent;

  use super::{Shortcut, is, numbered};

  /// Cmd+T, Cmd+W, Cmd+Shift+[ / ], Cmd+1…9, and history on Cmd+[ / Cmd+], which edit nothing in
  /// a text field, so they work while one has the focus.
  pub(super) fn shortcut(event: &KeyboardEvent) -> Option<Shortcut> {
    if !event.meta || event.alt || event.ctrl {
      return None;
    }
    let left = is(event, "[", "BracketLeft") || is(event, "{", "BracketLeft");
    let right = is(event, "]", "BracketRight") || is(event, "}", "BracketRight");
    match (event.shift, left, right) {
      (true, true, _) => return Some(Shortcut::PreviousTab),
      (true, _, true) => return Some(Shortcut::NextTab),
      (false, true, _) => return Some(Shortcut::Back),
      (false, _, true) => return Some(Shortcut::Forward),
      _ => {}
    }
    if event.shift {
      return None;
    }
    if is(event, "t", "KeyT") {
      return Some(Shortcut::NewTab);
    }
    if is(event, "w", "KeyW") {
      return Some(Shortcut::CloseTab);
    }
    numbered(event)
  }
}

#[cfg(not(target_os = "macos"))]
mod platform {
  use lurq::app::events::KeyboardEvent;

  use super::{Shortcut, is, numbered};

  /// Ctrl+T, Ctrl+W, Ctrl+PageDown / PageUp, Ctrl+1…9, and history on Alt+Left / Alt+Right, which
  /// stay with a focused text field.
  pub(super) fn shortcut(event: &KeyboardEvent) -> Option<Shortcut> {
    if event.alt && !event.ctrl && !event.meta && !event.shift && !event.text_input_focused {
      if is(event, "ArrowLeft", "ArrowLeft") {
        return Some(Shortcut::Back);
      }
      if is(event, "ArrowRight", "ArrowRight") {
        return Some(Shortcut::Forward);
      }
      return None;
    }
    if !event.ctrl || event.alt || event.meta || event.shift {
      return None;
    }
    if is(event, "PageDown", "PageDown") {
      return Some(Shortcut::NextTab);
    }
    if is(event, "PageUp", "PageUp") {
      return Some(Shortcut::PreviousTab);
    }
    if is(event, "t", "KeyT") {
      return Some(Shortcut::NewTab);
    }
    if is(event, "w", "KeyW") {
      return Some(Shortcut::CloseTab);
    }
    numbered(event)
  }
}
