//! The form's keys: Ctrl+Enter (⌘ Enter on macOS) asks for Create & run, Ctrl+S (⌘ S) for Create or Save changes,
//! wherever the focus is in the form, also in a field. A press while the action is held back does nothing but, for a
//! run, has the chips the run needs say their names in `warning`. lurq hands a key to every node from the window's
//! root down before a focused field takes it, so the form's root handles them and keeps them from the fields.

use lurq::{app::events::keyboard_event::KeyboardEvent, core::Signal};

use super::bar::Handler;
use crate::navigation::shortcuts::Keys;

/// What the keys do now.
pub struct FormKeys {
  /// Create & run; `None` while it is held back or the task offers none.
  pub run: Option<Handler>,
  /// Whether the task offers Create & run at all.
  pub offers_run: bool,
  /// Create or Save changes; `None` while it is held back.
  pub save: Option<Handler>,
  /// The keys are the form's: no question is asked in place of the bar and no newer version is being chosen from.
  pub active: bool,
  pub tried_run: Signal<bool>,
}

impl FormKeys {
  pub fn down(&self, event: &KeyboardEvent) {
    let keys = Keys::of(event);
    if !self.active || event.composing || event.default_prevented() || !keys.only_command() {
      return;
    }
    if keys.is("Enter", "Enter") && self.offers_run {
      event.prevent_default();
      match &self.run {
        Some(run) => run(),
        None => self.tried_run.set(true),
      }
    } else if keys.is("s", "KeyS") {
      event.prevent_default();
      if let Some(save) = &self.save {
        save();
      }
    }
  }
}
