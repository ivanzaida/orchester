//! The person's input on a tab, as the window reads it from lurq's pointer, wheel, keyboard, text and IME events; the
//! runner turns it into `Input.dispatchMouseEvent`, `Input.dispatchKeyEvent`, `Input.insertText` and
//! `Input.imeSetComposition` (`docs/design/browser.md`, "The person's input"). Coordinates are the page's CSS pixels
//! from the page area's top left corner.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PersonInput {
  Mouse {
    action: MouseAction,
    x: i32,
    y: i32,
    button: MouseButton,
    /// 1 for a click, 2 for the second press of a double click.
    clicks: u8,
    modifiers: Modifiers,
  },
  Wheel {
    x: i32,
    y: i32,
    delta_x: i32,
    delta_y: i32,
    modifiers: Modifiers,
  },
  /// A key pressed or released: `key` and `code` as the DOM names them (`Enter`, `KeyA`), and the text a press types,
  /// if any. Typed text without a key of its own comes as [`PersonInput::Text`].
  Key {
    action: KeyAction,
    key: String,
    code: String,
    text: Option<String>,
    modifiers: Modifiers,
  },
  /// Text committed by the keyboard or an input method, or pasted from the clipboard, inserted at the caret.
  Text { text: String },
  /// An input method's composition under way: its text so far and its selection, in characters.
  Composition {
    text: String,
    selection_start: u32,
    selection_end: u32,
  },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseAction {
  Pressed,
  Released,
  Moved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
  None,
  Left,
  Middle,
  Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyAction {
  Down,
  Up,
}

/// The modifier keys held during an input.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Modifiers {
  pub alt: bool,
  pub ctrl: bool,
  pub meta: bool,
  pub shift: bool,
}

impl Modifiers {
  /// The modifiers as CDP's input methods take them: Alt 1, Ctrl 2, Meta (⌘) 4, Shift 8.
  pub fn bits(self) -> u8 {
    u8::from(self.alt) | u8::from(self.ctrl) << 1 | u8::from(self.meta) << 2 | u8::from(self.shift) << 3
  }
}
