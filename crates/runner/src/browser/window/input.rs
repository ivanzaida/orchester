//! The person's input as CDP's input methods take it (`docs/design/browser.md`, "The person's input"): mouse and
//! wheel through `Input.dispatchMouseEvent`, keys through `Input.dispatchKeyEvent`, committed text through
//! `Input.insertText`, and an input method's composition through `Input.imeSetComposition`.

use orchester_ipc::browser::{KeyAction, Modifiers, MouseAction, MouseButton, PersonInput, SelectList};
use serde_json::json;

use super::select;
use crate::browser::{
  BrowserError,
  cdp::{Cdp, SessionId},
};

/// Sends `input` to the tab of `session`. A left press on a `<select>` is not sent: its list, which a headless frame
/// does not show, is returned for the window to draw.
pub(super) async fn dispatch(
  cdp: &Cdp,
  session: &SessionId,
  input: &PersonInput,
) -> Result<Option<SelectList>, BrowserError> {
  let (method, params) = match input {
    PersonInput::Mouse {
      action,
      x,
      y,
      button,
      clicks,
      modifiers,
    } => {
      if *action == MouseAction::Pressed
        && *button == MouseButton::Left
        && let Some(list) = select::at(cdp, session, *x, *y).await?
      {
        return Ok(Some(list));
      }
      let kind = match action {
        MouseAction::Pressed => "mousePressed",
        MouseAction::Released => "mouseReleased",
        MouseAction::Moved => "mouseMoved",
      };
      let params = json!({
        "type": kind,
        "x": x,
        "y": y,
        "button": button_name(*button),
        "clickCount": clicks,
        "modifiers": modifiers.bits(),
      });
      ("Input.dispatchMouseEvent", params)
    }
    PersonInput::Wheel {
      x,
      y,
      delta_x,
      delta_y,
      modifiers,
    } => {
      let params = json!({
        "type": "mouseWheel",
        "x": x,
        "y": y,
        "deltaX": delta_x,
        "deltaY": delta_y,
        "modifiers": modifiers.bits(),
      });
      ("Input.dispatchMouseEvent", params)
    }
    PersonInput::Key {
      action,
      key,
      code,
      text,
      modifiers,
    } => (
      "Input.dispatchKeyEvent",
      key_event(*action, key, code, text.as_deref(), *modifiers),
    ),
    PersonInput::Text { text } => ("Input.insertText", json!({ "text": text })),
    PersonInput::Composition {
      text,
      selection_start,
      selection_end,
    } => {
      let params = json!({ "text": text, "selectionStart": selection_start, "selectionEnd": selection_end });
      ("Input.imeSetComposition", params)
    }
  };
  cdp.call(Some(session), method, params).await?;
  Ok(None)
}

fn button_name(button: MouseButton) -> &'static str {
  match button {
    MouseButton::None => "none",
    MouseButton::Left => "left",
    MouseButton::Middle => "middle",
    MouseButton::Right => "right",
  }
}

/// A key press types its text (`keyDown`), or does only what the key does (`rawKeyDown`: arrows, Backspace); Enter
/// types a carriage return, as a form's submit expects. The page reads the key's code for keys without text.
fn key_event(action: KeyAction, key: &str, code: &str, text: Option<&str>, modifiers: Modifiers) -> serde_json::Value {
  let text = text.or((key == "Enter").then_some("\r"));
  let kind = match (action, text) {
    (KeyAction::Up, _) => "keyUp",
    (KeyAction::Down, Some(_)) => "keyDown",
    (KeyAction::Down, None) => "rawKeyDown",
  };
  let mut params = json!({ "type": kind, "key": key, "code": code, "modifiers": modifiers.bits() });
  if let Some(code) = key_code(key) {
    params["windowsVirtualKeyCode"] = json!(code);
    params["nativeVirtualKeyCode"] = json!(code);
  }
  if let (KeyAction::Down, Some(text)) = (action, text) {
    params["text"] = json!(text);
    params["unmodifiedText"] = json!(text);
  }
  params
}

/// The Windows virtual key code Chromium reads for `key`, a DOM key name; `None` for a key without one here.
fn key_code(key: &str) -> Option<u16> {
  let named = match key {
    "Backspace" => Some(8),
    "Tab" => Some(9),
    "Enter" => Some(13),
    "Shift" => Some(16),
    "Control" => Some(17),
    "Alt" => Some(18),
    "Escape" => Some(27),
    " " => Some(32),
    "PageUp" => Some(33),
    "PageDown" => Some(34),
    "End" => Some(35),
    "Home" => Some(36),
    "ArrowLeft" => Some(37),
    "ArrowUp" => Some(38),
    "ArrowRight" => Some(39),
    "ArrowDown" => Some(40),
    "Delete" => Some(46),
    "Meta" => Some(91),
    _ => None,
  };
  let function = || {
    let number = key.strip_prefix('F')?.parse::<u16>().ok()?;
    (1..=12).contains(&number).then_some(111 + number)
  };
  let character = || {
    let mut chars = key.chars();
    match (chars.next(), chars.next()) {
      (Some(letter), None) if letter.is_ascii_alphanumeric() => Some(u16::from(letter.to_ascii_uppercase() as u8)),
      _ => None,
    }
  };
  named.or_else(function).or_else(character)
}
