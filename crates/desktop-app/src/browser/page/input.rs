//! The person's input on the page (`docs/design/browser.md`, "The person's input"): lurq's pointer, wheel and keyboard
//! events on the page become `PersonInput`s in the page's CSS pixels, sent to the tab shown. A press focuses the page;
//! keys reach the tab only while the page has the focus, so typing in the address line stays there, and a key the page
//! takes (Tab, arrows, Backspace) does not also move the window's focus. A key that types sends its text with it.
//! Each input marks the tab as used, so its agent waits (`02b` `k7xvx`). An input method's composition is not sent
//! yet: lurq gives it only to a text input of its own; what it commits arrives as keys.

use std::{
  sync::Arc,
  time::{Duration, Instant},
};

use lurq::{
  app::{
    ctx::Ctx,
    events::{KeyboardEvent, MouseButton as Button, MouseEvent, ScrollEvent},
  },
  components::Stack,
  core::ElementRef,
};
use orchester_ipc::browser::{
  BrowserCommand, BrowserTabId, KeyAction, Modifiers, MouseAction, MouseButton, PersonInput,
};
use parking_lot::Mutex;

use crate::browser::state::Showing;

/// Two presses this close in time are a double click.
const DOUBLE_CLICK: Duration = Duration::from_millis(500);

/// What the page remembers of the pointer between events.
#[derive(Clone)]
pub struct PageInput {
  pressed: Arc<Mutex<Pressed>>,
}

#[derive(Default)]
struct Pressed {
  /// The button held, sent with each move.
  held: Option<MouseButton>,
  /// When the last press was, for a double click.
  last: Option<Instant>,
}

impl PageInput {
  pub fn new(_ctx: &mut Ctx) -> Self {
    Self {
      pressed: Arc::new(Mutex::new(Pressed::default())),
    }
  }

  /// `page` with the handlers that send the person's input to `tab`; `css` is the frame's size in CSS pixels, once a
  /// frame came.
  pub fn attach(
    &self,
    page: Stack,
    (showing, tab): (&Showing, &BrowserTabId),
    (element, css): (ElementRef, Option<(u32, u32)>),
  ) -> Stack {
    let send = Sender {
      showing: showing.clone(),
      tab: tab.clone(),
      element,
      css,
    };
    let (down, up, moved, wheel, key_down, key_up) = (
      (send.clone(), self.pressed.clone()),
      (send.clone(), self.pressed.clone()),
      (send.clone(), self.pressed.clone()),
      send.clone(),
      send.clone(),
      send,
    );
    page
      .focusable(true)
      .on_mouse_down(move |event: MouseEvent| {
        let (send, pressed) = &down;
        let button = button(event.button);
        let clicks = {
          let mut pressed = pressed.lock();
          let now = Instant::now();
          let double = pressed.last.is_some_and(|last| now.duration_since(last) < DOUBLE_CLICK);
          pressed.held = Some(button);
          pressed.last = if double { None } else { Some(now) };
          if double { 2 } else { 1 }
        };
        send.mouse(&event, (MouseAction::Pressed, button, clicks));
      })
      .on_mouse_up(move |event: MouseEvent| {
        let (send, pressed) = &up;
        pressed.lock().held = None;
        send.mouse(&event, (MouseAction::Released, button(event.button), 1));
      })
      .on_mouse_move(move |event: MouseEvent| {
        let (send, pressed) = &moved;
        let held = pressed.lock().held.unwrap_or(MouseButton::None);
        send.mouse(&event, (MouseAction::Moved, held, 0));
      })
      .on_scroll(move |event: ScrollEvent| {
        event.prevent_default();
        wheel.wheel(&event);
      })
      .on_key_down(move |event: KeyboardEvent| key_down.key(&event, KeyAction::Down))
      .on_key_up(move |event: KeyboardEvent| key_up.key(&event, KeyAction::Up))
  }
}

/// Sends the person's input on one tab.
#[derive(Clone)]
struct Sender {
  showing: Showing,
  tab: BrowserTabId,
  element: ElementRef,
  css: Option<(u32, u32)>,
}

impl Sender {
  fn send(&self, input: PersonInput, used: bool) {
    if used {
      self.showing.person.used(&self.tab);
    }
    self.showing.commands.send_quietly(BrowserCommand::Input {
      tab: self.tab.clone(),
      input,
    });
  }

  /// The point `(x, y)` of the window in the page's CSS pixels.
  fn point(&self, x: f32, y: f32) -> Option<(i32, i32)> {
    let (css_width, css_height) = self.css?;
    let bounds = self.element.bounds();
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
      return None;
    }
    let across = (x - bounds.x) / bounds.width * css_width as f32;
    let down = (y - bounds.y) / bounds.height * css_height as f32;
    Some((across.round() as i32, down.round() as i32))
  }

  fn mouse(&self, event: &MouseEvent, (action, button, clicks): (MouseAction, MouseButton, u8)) {
    let Some((x, y)) = self.point(event.x, event.y) else {
      return;
    };
    let input = PersonInput::Mouse {
      action,
      x,
      y,
      button,
      clicks,
      modifiers: modifiers(event.alt, event.ctrl, event.meta, event.shift),
    };
    self.send(input, action != MouseAction::Moved);
  }

  /// lurq's wheel delta is how far the content moves (up for a positive delta); the page's is how far it scrolls.
  fn wheel(&self, event: &ScrollEvent) {
    let Some((x, y)) = self.point(event.x, event.y) else {
      return;
    };
    let input = PersonInput::Wheel {
      x,
      y,
      delta_x: (-event.delta_x).round() as i32,
      delta_y: (-event.delta_y).round() as i32,
      modifiers: Modifiers::default(),
    };
    self.send(input, true);
  }

  fn key(&self, event: &KeyboardEvent, action: KeyAction) {
    if !self.element.focused() || event.text_input_focused {
      return;
    }
    event.prevent_default();
    let shortcut = event.ctrl || event.meta;
    let text = (action == KeyAction::Down && !shortcut && event.key.chars().count() == 1).then(|| event.key.clone());
    let input = PersonInput::Key {
      action,
      key: event.key.clone(),
      code: event.code.clone(),
      text,
      modifiers: modifiers(event.alt, event.ctrl, event.meta, event.shift),
    };
    self.send(input, action == KeyAction::Down);
  }
}

const fn button(button: Button) -> MouseButton {
  match button {
    Button::Left => MouseButton::Left,
    Button::Middle => MouseButton::Middle,
    Button::Right => MouseButton::Right,
    _ => MouseButton::None,
  }
}

const fn modifiers(alt: bool, ctrl: bool, meta: bool, shift: bool) -> Modifiers {
  Modifiers { alt, ctrl, meta, shift }
}
