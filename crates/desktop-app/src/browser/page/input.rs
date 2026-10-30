//! The person's input on the page (`docs/design/browser.md`, "The person's input"): lurq's pointer, wheel and keyboard
//! events on the page become `PersonInput`s in the page's CSS pixels, sent to the tab shown. A press gives the page the
//! focus, which a hidden text input holds for it; keys reach the tab only while it has the focus, so typing in the
//! address line stays there, and a key the page takes (Tab, arrows, Backspace) does not also move the window's focus.
//! A key that types sends its text with it. The hidden input is there for input methods, which lurq lets compose only
//! in a focused text input: the text one commits is sent as `PersonInput::Text`, and its candidates show where the
//! page was pressed. A composition under way is not sent, as lurq shows it in the input and tells no handler of it;
//! nor is a key event of the composition, though the text of a key the input method passes on goes as text, which lurq
//! types in the input. Each input marks the tab as used, so its agent waits (`02b` `k7xvx`).

use std::{
  sync::Arc,
  time::{Duration, Instant},
};

use lurq::{
  app::{
    ctx::Ctx,
    events::{KeyboardEvent, MouseButton as Button, MouseEvent, ScrollEvent, TextInputEvent},
  },
  components::{Stack, TextInput},
  core::{ElementRef, Signal},
  node::{HitTestBehavior, dimension::Dimension},
};
use orchester_ipc::browser::{
  BrowserCommand, BrowserTabId, KeyAction, Modifiers, MouseAction, MouseButton, PersonInput,
};
use parking_lot::Mutex;

use crate::browser::state::Showing;

/// Two presses this close in time are a double click.
const DOUBLE_CLICK: Duration = Duration::from_millis(500);

/// The hidden input is not seen, nor its caret or a composition in it; one pixel square, so lurq lays it out and gives
/// an input method its place.
const HIDDEN_OPACITY: f32 = 0.0;
const HIDDEN_SIZE: f32 = 1.0;

/// What the page remembers of the pointer between events, and the hidden input that holds its focus.
#[derive(Clone)]
pub struct PageInput {
  pressed: Arc<Mutex<Pressed>>,
  text: ElementRef,
  /// The hidden input's value, kept empty: what it would take goes to the tab.
  value: Signal<String>,
  /// Where the page was last pressed, from its top left, where the hidden input stands.
  at: Signal<(f32, f32)>,
  /// A press asked for the page's focus, which the next render gives the hidden input.
  focus: Signal<bool>,
}

#[derive(Default)]
struct Pressed {
  /// The button held, sent with each move.
  held: Option<MouseButton>,
  /// When the last press was, for a double click.
  last: Option<Instant>,
  /// The last press's click count, which its release carries: the page fires `dblclick` on the release.
  clicks: u8,
}

impl PageInput {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      pressed: Arc::new(Mutex::new(Pressed::default())),
      text: ctx.element_ref(),
      value: ctx.signal(String::new()),
      at: ctx.signal((0.0, 0.0)),
      focus: ctx.signal(false),
    }
  }

  /// `page` with the handlers that send the person's input to `tab`, and the hidden input; `css` is the frame's size
  /// in CSS pixels, once a frame came.
  pub fn attach(
    &self,
    ctx: &mut Ctx,
    page: Stack,
    (showing, tab): (&Showing, &BrowserTabId),
    (element, css): (ElementRef, Option<(u32, u32)>),
  ) -> Stack {
    if self.focus.get() {
      ctx.focus(&self.text);
      self.focus.set(false);
    }
    let send = Sender {
      showing: showing.clone(),
      tab: tab.clone(),
      element,
      css,
      text: self.text.clone(),
    };
    let (down, up, moved) = (
      (send.clone(), self.clone()),
      (send.clone(), self.pressed.clone()),
      (send.clone(), self.pressed.clone()),
    );
    let (wheel, key_down, key_up, typed) = (send.clone(), send.clone(), send.clone(), send);
    let (x, y) = self.at.get();
    let hidden = TextInput::new(self.value.clone())
      .id("browser-page-text")
      .ref_element(self.text.clone())
      .absolute(x, y, Dimension::Px(HIDDEN_SIZE), Dimension::Px(HIDDEN_SIZE))
      .hit_test(HitTestBehavior::None)
      .opacity(HIDDEN_OPACITY)
      .single_line()
      .on_input(move |event: TextInputEvent| typed.committed(&event));
    page
      .on_mouse_down(move |event: MouseEvent| {
        let (send, input) = &down;
        input.pressed_at(send, &event);
        let button = button(event.button);
        let clicks = {
          let mut pressed = input.pressed.lock();
          let now = Instant::now();
          let double = pressed.last.is_some_and(|last| now.duration_since(last) < DOUBLE_CLICK);
          pressed.held = Some(button);
          pressed.last = if double { None } else { Some(now) };
          pressed.clicks = if double { 2 } else { 1 };
          pressed.clicks
        };
        send.mouse(&event, (MouseAction::Pressed, button, clicks));
      })
      .on_mouse_up(move |event: MouseEvent| {
        let (send, pressed) = &up;
        let clicks = {
          let mut pressed = pressed.lock();
          pressed.held = None;
          pressed.clicks.max(1)
        };
        send.mouse(&event, (MouseAction::Released, button(event.button), clicks));
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
      .child(hidden)
  }

  /// A press on the page moves the hidden input there and asks for the focus.
  fn pressed_at(&self, send: &Sender, event: &MouseEvent) {
    let bounds = send.element.bounds();
    self.at.set((event.x - bounds.x, event.y - bounds.y));
    self.focus.set(true);
  }
}

/// Sends the person's input on one tab.
#[derive(Clone)]
struct Sender {
  showing: Showing,
  tab: BrowserTabId,
  element: ElementRef,
  css: Option<(u32, u32)>,
  /// The hidden input, whose focus is the page's.
  text: ElementRef,
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

  /// A key the window took for a shortcut (`browser::keys`) does not reach the page, nor one an input method's
  /// composition takes: what the composition commits comes as text (`Self::committed`).
  fn key(&self, event: &KeyboardEvent, action: KeyAction) {
    if !self.text.focused() || event.default_prevented() || event.composing {
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

  /// The text an input method committed or passed on in the hidden input goes to the tab; the input keeps nothing.
  fn committed(&self, event: &TextInputEvent) {
    event.prevent_default();
    if event.keyboard.composing && !event.keyboard.key.is_empty() {
      let text = event.keyboard.key.clone();
      self.send(PersonInput::Text { text }, true);
    }
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
