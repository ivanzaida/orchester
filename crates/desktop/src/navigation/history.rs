//! Back and forward from the mouse's side buttons and the platform's history shortcuts, handled
//! for the whole window. The shell reports the side buttons as `Other(4)` (back) and `Other(5)`
//! (forward); they act on release, as a browser does. Shortcuts: Alt+Left / Alt+Right on
//! Windows and Linux, Cmd+[ / Cmd+] on macOS. Nothing happens for an event a control already
//! handled, and Alt+Left / Alt+Right stay with a focused text field.

use lurq::{
  app::events::{KeyboardEvent, MouseButton, MouseEvent},
  components::Column,
  node::{Element, dimension::Dimension},
  router::RouterHandle,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
  Back,
  Forward,
}

const BACK_BUTTON: MouseButton = MouseButton::Other(4);
const FORWARD_BUTTON: MouseButton = MouseButton::Other(5);

/// `content` filling the window, with the history input around it. The key listener is the
/// last node of the tree: keyboard events visit nodes from the root down in order, so every
/// control has seen the key before it and can claim it with `prevent_default`.
pub fn with_history_input(router: &RouterHandle, content: Element) -> Element {
  let (mouse, keys) = (router.clone(), router.clone());
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .on_mouse_up(move |event: MouseEvent| {
      if event.default_prevented() {
        return;
      }
      if let Some(step) = mouse_step(event.button) {
        event.prevent_default();
        go(&mouse, step);
      }
    })
    .child(content)
    .child(Column::new().on_key_down(move |event: KeyboardEvent| {
      if event.default_prevented() {
        return;
      }
      if let Some(step) = key_step(&event) {
        event.prevent_default();
        go(&keys, step);
      }
    }))
    .into()
}

fn go(router: &RouterHandle, step: Step) {
  // At either end of the history there is nowhere to go, and the step does nothing.
  match step {
    Step::Back => router.back(),
    Step::Forward => router.forward(),
  };
}

fn mouse_step(button: MouseButton) -> Option<Step> {
  match button {
    BACK_BUTTON => Some(Step::Back),
    FORWARD_BUTTON => Some(Step::Forward),
    _ => None,
  }
}

/// Cmd+[ and Cmd+] edit nothing in a text field, so they work while one has the focus.
#[cfg(target_os = "macos")]
fn key_step(event: &KeyboardEvent) -> Option<Step> {
  if !event.meta || event.alt || event.ctrl || event.shift {
    return None;
  }
  match (event.key.as_str(), event.code.as_str()) {
    ("[", _) | (_, "BracketLeft") => Some(Step::Back),
    ("]", _) | (_, "BracketRight") => Some(Step::Forward),
    _ => None,
  }
}

/// Alt+Left and Alt+Right move the caret by words in a focused text field, so they are left to it.
#[cfg(not(target_os = "macos"))]
fn key_step(event: &KeyboardEvent) -> Option<Step> {
  if !event.alt || event.ctrl || event.meta || event.shift || event.text_input_focused {
    return None;
  }
  match (event.key.as_str(), event.code.as_str()) {
    ("ArrowLeft", _) | (_, "ArrowLeft") => Some(Step::Back),
    ("ArrowRight", _) | (_, "ArrowRight") => Some(Step::Forward),
    _ => None,
  }
}
