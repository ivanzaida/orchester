//! Keyboard and wheel input of the canvas zoom. The zoom modifier is Ctrl, or Cmd on macOS:
//! with it the wheel zooms instead of panning, and `+`/`=`, `-` and `0` zoom in, out and back
//! to 100 %. The shortcuts stay with a focused text field, which edits with them.
//!
//! lurq's wheel events carry no modifier keys, so whether the modifier is held is followed from
//! the key events (and, where they carry it, pointer events) the canvas sees.

use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

use lurq::app::events::{KeyboardEvent, MouseEvent};

/// A change of zoom asked for by a zoom control or a shortcut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoomCommand {
  In,
  Out,
  /// Back to 100 %.
  Full,
  /// The whole pipeline in view.
  Fit,
}

/// The zoom shortcut `event` presses, if any.
pub fn shortcut(event: &KeyboardEvent) -> Option<ZoomCommand> {
  if event.text_input_focused || !platform::only_modifier(event) {
    return None;
  }
  match (event.key.as_str(), event.code.as_str()) {
    ("=" | "+", _) | (_, "Equal" | "NumpadAdd") => Some(ZoomCommand::In),
    ("-", _) | (_, "Minus" | "NumpadSubtract") => Some(ZoomCommand::Out),
    ("0", _) | (_, "Digit0" | "Numpad0") => Some(ZoomCommand::Full),
    _ => None,
  }
}

/// Whether the zoom modifier is held, as the latest key and pointer events tell.
#[derive(Clone, Default)]
pub struct ZoomModifier(Arc<AtomicBool>);

impl ZoomModifier {
  pub fn held(&self) -> bool {
    self.0.load(Ordering::Relaxed)
  }

  pub fn key_down(&self, event: &KeyboardEvent) {
    let held = platform::is_modifier_key(&event.key) || platform::modifier(event);
    self.0.store(held, Ordering::Relaxed);
  }

  pub fn key_up(&self, event: &KeyboardEvent) {
    let held = !platform::is_modifier_key(&event.key) && platform::modifier(event);
    self.0.store(held, Ordering::Relaxed);
  }

  /// Pointer events tell the modifier where the platform's pointer events carry it.
  pub fn pointer(&self, event: &MouseEvent) {
    if let Some(held) = platform::pointer_modifier(event) {
      self.0.store(held, Ordering::Relaxed);
    }
  }
}

#[cfg(target_os = "macos")]
mod platform {
  use lurq::app::events::{KeyboardEvent, MouseEvent};

  /// winit names Cmd `Super`.
  pub fn is_modifier_key(key: &str) -> bool {
    matches!(key, "Super" | "Meta")
  }

  pub fn modifier(event: &KeyboardEvent) -> bool {
    event.meta
  }

  pub fn only_modifier(event: &KeyboardEvent) -> bool {
    event.meta && !event.ctrl && !event.alt
  }

  /// lurq's pointer events carry no Cmd; the key events alone tell it.
  pub fn pointer_modifier(_event: &MouseEvent) -> Option<bool> {
    None
  }
}

#[cfg(not(target_os = "macos"))]
mod platform {
  use lurq::app::events::{KeyboardEvent, MouseEvent};

  pub fn is_modifier_key(key: &str) -> bool {
    key == "Control"
  }

  pub fn modifier(event: &KeyboardEvent) -> bool {
    event.ctrl
  }

  pub fn only_modifier(event: &KeyboardEvent) -> bool {
    event.ctrl && !event.meta && !event.alt
  }

  pub fn pointer_modifier(event: &MouseEvent) -> Option<bool> {
    Some(event.ctrl)
  }
}
