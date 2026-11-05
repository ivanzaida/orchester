//! `browser_key`: key presses as the browser gets a person's, each its key down and up with the modifiers held. A key
//! is one character, or a key's name (`Enter`, `ArrowDown`, `F5`); a character types itself unless Ctrl, Alt or Meta is
//! held.

use orchester_core::orchester_tools::browser::{KeyPress, Modifier};
use serde_json::{Value, json};

use super::ToolError;
use crate::browser::Tab;

/// The named keys: name, `code`, Windows virtual key code, and the text the key types.
const NAMED: &[(&str, &str, i64, Option<&str>)] = &[
  ("Enter", "Enter", 13, Some("\r")),
  ("Tab", "Tab", 9, None),
  ("Escape", "Escape", 27, None),
  ("Backspace", "Backspace", 8, None),
  ("Delete", "Delete", 46, None),
  ("Insert", "Insert", 45, None),
  ("Space", "Space", 32, Some(" ")),
  ("ArrowUp", "ArrowUp", 38, None),
  ("ArrowDown", "ArrowDown", 40, None),
  ("ArrowLeft", "ArrowLeft", 37, None),
  ("ArrowRight", "ArrowRight", 39, None),
  ("Home", "Home", 36, None),
  ("End", "End", 35, None),
  ("PageUp", "PageUp", 33, None),
  ("PageDown", "PageDown", 34, None),
  ("Shift", "ShiftLeft", 16, None),
  ("Control", "ControlLeft", 17, None),
  ("Alt", "AltLeft", 18, None),
  ("Meta", "MetaLeft", 91, None),
];
/// The function keys, `F1` to `F12`.
const FUNCTION_KEYS: u8 = 12;
/// The Windows virtual key code of `F1`; the others follow it.
const F1_CODE: i64 = 112;

/// One key as CDP's `Input.dispatchKeyEvent` names it.
struct Key {
  key: String,
  code: String,
  virtual_code: i64,
  text: Option<String>,
}

/// Presses `keys` in `tab`, one after the other.
pub(super) async fn press(tab: &Tab, keys: &[KeyPress]) -> Result<String, ToolError> {
  for press in keys {
    let key = key_of(press.key())?;
    let modifiers = press.modifiers().iter().fold(0, |bits, modifier| bits | bit(*modifier));
    let shortcut = press.modifiers().iter().any(|modifier| *modifier != Modifier::Shift);
    let text = key.text.as_deref().filter(|_| !shortcut);
    let mut down = json!({
      "type": if text.is_some() { "keyDown" } else { "rawKeyDown" },
      "modifiers": modifiers,
      "key": key.key,
      "code": key.code,
      "windowsVirtualKeyCode": key.virtual_code,
    });
    if let Some(text) = text {
      down["text"] = Value::from(text);
      down["unmodifiedText"] = Value::from(text);
    }
    tab.call("Input.dispatchKeyEvent", down).await?;
    tab
      .call(
        "Input.dispatchKeyEvent",
        json!({
          "type": "keyUp",
          "modifiers": modifiers,
          "key": key.key,
          "code": key.code,
          "windowsVirtualKeyCode": key.virtual_code,
        }),
      )
      .await?;
  }
  let names: Vec<String> = keys.iter().map(shown).collect();
  Ok(format!("Pressed {}.", names.join(", ")))
}

/// The key named `name`: one character, a named key, or a function key.
fn key_of(name: &str) -> Result<Key, ToolError> {
  let mut chars = name.chars();
  if let (Some(c), None) = (chars.next(), chars.next()) {
    let (code, virtual_code) = match c {
      'a'..='z' | 'A'..='Z' => (
        format!("Key{}", c.to_ascii_uppercase()),
        i64::from(c.to_ascii_uppercase() as u8),
      ),
      '0'..='9' => (format!("Digit{c}"), i64::from(c as u8)),
      _ => (String::new(), 0),
    };
    return Ok(Key {
      key: c.to_string(),
      code,
      virtual_code,
      text: Some(c.to_string()),
    });
  }
  if let Some((key, code, virtual_code, text)) = NAMED.iter().find(|(key, ..)| key.eq_ignore_ascii_case(name)) {
    return Ok(Key {
      key: if *key == "Space" {
        " ".to_owned()
      } else {
        (*key).to_owned()
      },
      code: (*code).to_owned(),
      virtual_code: *virtual_code,
      text: text.map(str::to_owned),
    });
  }
  let function = name
    .strip_prefix(['F', 'f'])
    .and_then(|number| number.parse::<u8>().ok())
    .filter(|number| (1..=FUNCTION_KEYS).contains(number));
  match function {
    Some(number) => Ok(Key {
      key: format!("F{number}"),
      code: format!("F{number}"),
      virtual_code: F1_CODE + i64::from(number) - 1,
      text: None,
    }),
    None => Err(ToolError::Page(format!(
      "`{name}` is not a key the browser knows: use one character, F1 to F12, or one of {}",
      NAMED.iter().map(|(key, ..)| *key).collect::<Vec<_>>().join(", ")
    ))),
  }
}

/// The bit of `modifier` in CDP's `modifiers`.
const fn bit(modifier: Modifier) -> i64 {
  match modifier {
    Modifier::Alt => 1,
    Modifier::Ctrl => 2,
    Modifier::Meta => 4,
    Modifier::Shift => 8,
  }
}

/// `press` as the answer names it: `Ctrl+A`.
fn shown(press: &KeyPress) -> String {
  let mut parts: Vec<&str> = press
    .modifiers()
    .iter()
    .map(|modifier| match modifier {
      Modifier::Ctrl => "Ctrl",
      Modifier::Alt => "Alt",
      Modifier::Shift => "Shift",
      Modifier::Meta => "Meta",
    })
    .collect();
  parts.push(press.key());
  parts.join("+")
}
