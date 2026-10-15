//! The window's input for history and tabs, acting on the active tab. The mouse's side buttons go back
//! and forward on release, as a browser does (the shell reports them as `Other(4)` and `Other(5)`). The
//! key listener is the last node of the tree: keyboard events visit nodes from the root down in order,
//! so every control has seen a key before it and can claim it with `prevent_default`. The window's title
//! bar is not among them: lurq hands keys to the chrome's overlay, where it lies, after the page and this
//! listener. A part of it that must take keys first while it is open (the tab list) holds [`FirstKeys`],
//! which the window calls before anything else.

use std::sync::Arc;

use lurq::{
  app::{
    component::DevtoolsInspectable,
    events::{KeyboardEvent, MouseButton, MouseEvent},
  },
  components::Column,
  node::{Element, dimension::Dimension},
};
use parking_lot::Mutex;

use super::{
  Navigation,
  shortcuts::{Keys, Shortcut, shortcut},
};

const BACK_BUTTON: MouseButton = MouseButton::Other(4);
const FORWARD_BUTTON: MouseButton = MouseButton::Other(5);

/// What takes a key press.
pub type KeyHandler = Arc<dyn Fn(KeyboardEvent) + Send + Sync>;

/// The keys a part of the window takes before the page and the window's shortcuts, while it holds them.
#[derive(Clone, Default)]
pub struct FirstKeys(Arc<Mutex<Option<KeyHandler>>>);

/// One slot for the window's life.
impl PartialEq for FirstKeys {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl DevtoolsInspectable for FirstKeys {}

impl FirstKeys {
  /// `handler` takes every key first until [`release`](Self::release).
  pub fn hold(&self, handler: KeyHandler) {
    *self.0.lock() = Some(handler);
  }

  pub fn release(&self) {
    *self.0.lock() = None;
  }

  fn handle(&self, event: KeyboardEvent) {
    let handler = self.0.lock().clone();
    if let Some(handler) = handler {
      handler(event);
    }
  }
}

/// `content` filling the window, with the history and tab input around it while `active`; none while the
/// closing panel holds the window. `first` takes keys before all of it.
pub fn window_input(navigation: &Navigation, content: Element, (active, first): (bool, FirstKeys)) -> Column {
  let (mouse, keys) = (navigation.clone(), navigation.clone());
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .on_key_down(move |event: KeyboardEvent| {
      if active {
        first.handle(event);
      }
    })
    .on_mouse_up(move |event: MouseEvent| {
      if !active || event.default_prevented() {
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
    .child(Column::new().on_key_down(move |event: KeyboardEvent| {
      if !active || event.default_prevented() {
        return;
      }
      let Some(shortcut) = shortcut(Keys::of(&event)) else {
        return;
      };
      event.prevent_default();
      run(&keys, shortcut);
    }))
}

fn run(navigation: &Navigation, shortcut: Shortcut) {
  match shortcut {
    Shortcut::Back => navigation.back(),
    Shortcut::Forward => navigation.forward(),
    Shortcut::NewTab => navigation.open_tab(),
    Shortcut::CloseTab => navigation.close_active(),
    Shortcut::NextTab => navigation.cycle(1),
    Shortcut::PreviousTab => navigation.cycle(-1),
  }
}
