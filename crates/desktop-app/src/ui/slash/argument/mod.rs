//! A command that takes an argument (ORC-771, `Orchestrator watch · redesign` `QOPcg`): chosen in the slash menu, it
//! becomes the composer's command chip before the message and its picker opens over the composer's box. Enter adds
//! the picker's focused choice as a chip after it, several allowed; Backspace in an empty field removes the last
//! chip, the command's own once no choice is left; Esc closes the picker and keeps the chips, and a press on the
//! command's chip opens it again. The words typed after the chips filter the picker while it is open and are the
//! command's text; Ctrl Enter (Cmd Enter) applies the command to its choices and text instead of sending a message.
//! What a picker offers is the command's own ([`ArgumentPicker`]); the slash menu draws the chips and does the keys.

mod chips;
mod picker;

use std::sync::Arc;

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx, events::KeyboardEvent},
  node::Element,
};

pub(super) use self::{chips::chips, picker::picker};
use super::{SlashField, step};
use crate::ui::icon::Icon;

/// One of the things a command's argument can be, as its picker's row and its chip show it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
  /// What tells it from the other choices: what the command is applied to (a module's or a task's id), and what
  /// names its chip in tools and tests.
  pub key: Arc<str>,
  pub icon: Icon,
  pub label: Arc<str>,
}

/// What a command is applied to: the choices taken, in the order they were added, and the text written after them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Applied {
  pub choices: Vec<Choice>,
  pub text: String,
}

/// What applying a command does, and whether it took it: a command refused keeps its chips and text.
pub type Apply = Arc<dyn Fn(Applied) -> bool + Send + Sync>;

/// What a picker is asked to show now.
pub struct Asked<'a> {
  /// What names the composer: the picker's elements are named after it.
  pub id: &'a str,
  /// The words typed after the chips.
  pub filter: &'a str,
  /// The choices that are chips already.
  pub taken: &'a [Choice],
  /// The row with the keyboard's focus, among the rows the picker offered last for this filter.
  pub focused: Option<usize>,
}

/// What a picker shows: its rows as the keys read them, each with whether it can be chosen now, in order, and the
/// panel that draws them, the focused one marked as the slash menu marks its own.
pub struct Offer {
  pub rows: Vec<(Choice, bool)>,
  pub panel: Element,
}

/// The choices a command's argument can take, drawn in its picker. Each command with an argument brings its own (the
/// Orchestrator's `/watch-module` and `/watch-task` plug theirs in with ORC-772 and ORC-773).
pub trait ArgumentPicker: Send + Sync {
  /// The picker for `asked`; it is drawn in a component of its own, so it may read what it offers through `ctx`.
  fn offer(&self, ctx: &mut Ctx, asked: &Asked<'_>) -> Offer;
}

/// What a command that takes an argument does: its picker, and what applying it does.
#[derive(Clone)]
pub struct Taking {
  pub picker: Arc<dyn ArgumentPicker>,
  pub apply: Apply,
}

/// The command a composer's chip holds, and the choices after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Arguing {
  pub(super) command: &'static str,
  pub(super) icon: Icon,
  pub(super) choices: Vec<Choice>,
  /// Its picker is open.
  pub(super) picking: bool,
}

impl DevtoolsInspectable for Arguing {}

/// The rows the picker shows now, as the keys read them, and the one with the focus.
#[derive(Clone, Debug, Default)]
pub(super) struct Offered {
  pub(super) rows: Vec<(Choice, bool)>,
  pub(super) focused: Option<usize>,
}

impl SlashField {
  /// The name of the command the composer's chip holds, while it holds one, read tracked: the composer's placeholder
  /// is that command's then.
  pub fn held_command(&self) -> Option<&'static str> {
    self.argument.get().map(|held| held.command)
  }
  /// The command `name` becomes the chip, without choices, and its picker opens; any command held before goes.
  pub(super) fn hold(&self, (name, icon): (&'static str, Icon), taking: Taking) {
    *self.taking.lock() = Some(taking);
    self.forget_choices();
    self.argument.set(Some(Arguing {
      command: name,
      icon,
      choices: Vec::new(),
      picking: true,
    }));
  }

  /// Takes `event` while the chip holds a command, as the module's head says: whether it did.
  pub(super) fn argument_key(&self, event: &KeyboardEvent, held: &Arguing) -> bool {
    let plain = !event.shift && !event.ctrl && !event.meta && !event.alt;
    let took = match event.key.as_str() {
      "Enter" if (event.ctrl || event.meta) && !event.shift => {
        self.apply();
        true
      }
      "Enter" if held.picking && plain => {
        self.add_focused();
        true
      }
      "ArrowDown" | "ArrowUp" if held.picking => {
        self.move_focus(event.key == "ArrowDown");
        true
      }
      "Escape" if held.picking => {
        self.set_picking(false);
        true
      }
      "Backspace" if plain && self.draft.get_untracked().is_empty() => {
        self.remove_last();
        true
      }
      _ => false,
    };
    if took {
      event.prevent_default();
    }
    took
  }

  /// Opens or closes the picker of the command the chip holds.
  pub(super) fn set_picking(&self, picking: bool) {
    let Some(mut held) = self.argument.get_untracked() else {
      return;
    };
    if held.picking != picking {
      held.picking = picking;
      self.argument.set(Some(held));
    }
  }

  /// Removes the choice `key` from the chips.
  pub(super) fn remove_choice(&self, key: &str) {
    let Some(mut held) = self.argument.get_untracked() else {
      return;
    };
    held.choices.retain(|choice| &*choice.key != key);
    self.argument.set(Some(held));
  }

  /// Applies the command the chip holds to its choices and the text after them; once it took them, the chips and the
  /// text go, and they stay when it refused.
  fn apply(&self) {
    let Some(held) = self.argument.get_untracked() else {
      return;
    };
    let Some(taking) = self.taking.lock().clone() else {
      return;
    };
    let text = self.draft.get_untracked().trim().to_owned();
    let took = (taking.apply)(Applied {
      choices: held.choices,
      text,
    });
    if took {
      self.drop_command();
      self.draft.set(String::new());
    }
  }

  /// Enter in the picker: its focused row, while it can be chosen and is not a chip yet, becomes the last chip, and
  /// the filter empties.
  fn add_focused(&self) {
    let offered = self.offered_choices.lock().clone().unwrap_or_default();
    let Some((choice, true)) = offered.focused.and_then(|at| offered.rows.get(at)).cloned() else {
      return;
    };
    let Some(mut held) = self.argument.get_untracked() else {
      return;
    };
    if !held.choices.contains(&choice) {
      held.choices.push(choice);
      self.argument.set(Some(held));
    }
    if !self.draft.get_untracked().is_empty() {
      self.draft.set(String::new());
    }
  }

  /// ↑ ↓ in the picker: the row before or after the focused one that can be chosen, around the ends.
  fn move_focus(&self, down: bool) {
    let offered = self.offered_choices.lock().clone().unwrap_or_default();
    let choosable: Vec<bool> = offered.rows.iter().map(|(_, can)| *can).collect();
    let next = step(&choosable, offered.focused, down);
    if next != offered.focused {
      self.choice_focus.set(next);
    }
  }

  /// Backspace in an empty field: the last choice goes, or the command once none is left.
  fn remove_last(&self) {
    let Some(mut held) = self.argument.get_untracked() else {
      return;
    };
    if held.choices.pop().is_some() {
      self.argument.set(Some(held));
    } else {
      self.drop_command();
    }
  }

  /// The picker no longer shows: the keys have no row of it to move to or add.
  pub(super) fn forget_choices(&self) {
    *self.offered_choices.lock() = None;
  }

  fn drop_command(&self) {
    *self.taking.lock() = None;
    self.forget_choices();
    self.argument.set(None);
  }
}
