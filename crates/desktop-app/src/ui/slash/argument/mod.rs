//! A command that takes an argument (ORC-771; v2 in `Proposal · watch composer v2` `LRg3R` of `Orchestrator watch ·
//! redesign` `QOPcg`: the badge ORC-890, the picker `P1` `QXPDY` ORC-889): chosen in the slash menu, it becomes one
//! badge pinned in a row of its own at the top of the composer (*Watch · Orchestrator watch*, [`badges`]) and its
//! picker opens over the composer's box, the focus in the picker's own search field, whose words filter it
//! ([`PickerSearch`]). While the picker is open its keys are its own, wherever the focus is: ↑ ↓ move through the rows
//! that can be chosen, Enter adds the focused choice to the badge and closes the picker, Shift Enter adds it and keeps
//! the picker open, Esc closes it and keeps the badge; a press outside it closes it too, and a press on the badge opens
//! it again. Once it closes by its keys the composer's field takes the focus back. Backspace in an empty field removes
//! the open badge's last choice, and the badge once none is left. Once the picker is closed the composer is the normal
//! chat input, and a `/` opens the slash menu again, so one message can carry several commands, one badge each
//! (choosing a command held already opens its own badge's picker). Then Backspace at the start of the text removes the
//! last badge, and a badge's `x` removes it. The composer's own send applies every badge to its choices with the
//! message's text ([`SlashField::apply_held`]) and sends the message with them. What a picker offers is the command's
//! own ([`ArgumentPicker`]); the slash menu draws the badges and does the keys.

mod badges;
mod picker;
mod search;

use std::sync::{Arc, atomic::Ordering};

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx, events::KeyboardEvent},
  node::Element,
};
use orchester_core::id::ChatId;

pub use self::search::PickerSearch;
pub(super) use self::{badges::badges, picker::picker};
use super::{SlashField, step};
use crate::ui::icon::Icon;

/// One of the things a command's argument can be, as its picker's row and its badge show it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
  /// What tells it from the other choices: what the command is applied to (a module's or a task's id).
  pub key: Arc<str>,
  pub icon: Icon,
  pub label: Arc<str>,
}

/// What a command is applied to: the choices taken, in the order they were added; the text of the message that goes
/// with them; and the conversation that message goes to, a new one's id when it starts one (`None` where the
/// composer's screen names none).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Applied {
  pub choices: Vec<Choice>,
  pub text: String,
  pub chat: Option<ChatId>,
}

/// What applying a command does, and whether it took it: a command refused keeps its badge, and the message stays.
pub type Apply = Arc<dyn Fn(Applied) -> bool + Send + Sync>;

/// What a picker is asked to show now.
pub struct Asked<'a> {
  /// What names the composer: the picker's elements are named after it.
  pub id: &'a str,
  /// The words typed in the picker's own search field.
  pub filter: &'a str,
  /// The choices its badge holds already.
  pub taken: &'a [Choice],
  /// The row with the keyboard's focus, among the rows the picker offered last for this filter.
  pub focused: Option<usize>,
  /// The picker's search field, which its panel draws at its top, and what keeps its focused row in view.
  pub search: &'a PickerSearch,
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

/// What a command's badge says before its choices (`Badge · Watch · Orchestrator watch` `VvRWU`): its icon and verb.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Badge {
  pub icon: Icon,
  pub verb: Arc<str>,
}

/// What a command that takes an argument does: its picker, what applying it does, and its badge.
#[derive(Clone)]
pub struct Taking {
  pub picker: Arc<dyn ArgumentPicker>,
  pub apply: Apply,
  pub badge: Badge,
}

/// One badge of the composer: the command it holds and the choices taken for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Arguing {
  pub(super) command: &'static str,
  pub(super) badge: Badge,
  pub(super) choices: Vec<Choice>,
  /// Its picker is open; one badge's at most.
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
  /// Whether a badge holds a choice, read tracked: the composer's send then applies it, and Enter sends.
  pub fn holds_choices(&self) -> bool {
    self
      .argument
      .with(|held| held.iter().any(|badge| !badge.choices.is_empty()))
  }

  /// Backspace at the start of a written message, with no picker open: the last badge goes. Whether it took `event`.
  pub fn backspace_at_start(&self, event: &KeyboardEvent) -> bool {
    let plain = !event.shift && !event.ctrl && !event.meta && !event.alt;
    if event.key != "Backspace" || !plain || event.composing || event.default_prevented() {
      return false;
    }
    let Some(last) = self.argument.with_untracked(|held| {
      held
        .last()
        .filter(|_| !held.iter().any(|badge| badge.picking))
        .map(|badge| badge.command)
    }) else {
      return false;
    };
    self.remove_badge(last);
    event.prevent_default();
    true
  }

  /// The composer's send: applies each badge that holds a choice to its choices with `text`, the message's, which
  /// goes to conversation `chat`. Whether every one took them: each badge applied goes, and once all were the badges
  /// without a choice go too; a refused one stays with those after it, and nothing after it is applied.
  pub fn apply_held(&self, text: &str, chat: Option<ChatId>) -> bool {
    let held = self.argument.get_untracked();
    let mut left = held.clone();
    for badge in held.iter().filter(|badge| !badge.choices.is_empty()) {
      let Some(taking) = self.taking_of(badge.command) else {
        continue;
      };
      let took = (taking.apply)(Applied {
        choices: badge.choices.clone(),
        text: text.trim().to_owned(),
        chat,
      });
      if !took {
        self.keep(left);
        return false;
      }
      left.retain(|kept| kept.command != badge.command);
    }
    self.keep(Vec::new());
    true
  }

  /// The command `name` gets a badge, without choices, after those held, and its picker opens with an empty search;
  /// for a command held already, its own badge's picker opens.
  pub(super) fn hold(&self, name: &'static str, taking: Taking) {
    let badge = taking.badge.clone();
    {
      let mut takings = self.taking.lock();
      takings.retain(|(held, _)| *held != name);
      takings.push((name, taking));
    }
    self.forget_choices();
    self.clear_search();
    let mut held = self.argument.get_untracked();
    if !held.iter().any(|kept| kept.command == name) {
      held.push(Arguing {
        command: name,
        badge,
        choices: Vec::new(),
        picking: false,
      });
    }
    for kept in &mut held {
      kept.picking = kept.command == name;
    }
    self.argument.set(held);
  }

  /// What the badge of `command` does, while it is held.
  pub(super) fn taking_of(&self, command: &str) -> Option<Taking> {
    self
      .taking
      .lock()
      .iter()
      .find(|(held, _)| *held == command)
      .map(|(_, taking)| taking.clone())
  }

  /// Takes Backspace in an empty field while a badge is held, no picker open and the composer's field has the focus:
  /// whether it did. Enter is the composer's send.
  pub(super) fn argument_key(&self, event: &KeyboardEvent) -> bool {
    if self.argument.with_untracked(Vec::is_empty) {
      return false;
    }
    let took = match event.key.as_str() {
      "Backspace" if plain(event) && self.draft.get_untracked().is_empty() => {
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

  /// Takes `event` while a badge's picker is open, as the module's head says: whether it did.
  pub(super) fn picker_key(&self, event: &KeyboardEvent) -> bool {
    let shifted = event.shift && !event.ctrl && !event.meta && !event.alt;
    let took = match event.key.as_str() {
      "Enter" if plain(event) => {
        if self.add_focused() {
          self.close_picker();
        }
        true
      }
      "Enter" if shifted => {
        self.add_focused();
        true
      }
      "ArrowDown" | "ArrowUp" => {
        self.move_focus(event.key == "ArrowDown");
        true
      }
      "Escape" => {
        self.close_picker();
        true
      }
      "Backspace" if plain(event) && self.focused.get_untracked() && self.draft.get_untracked().is_empty() => {
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

  /// Opens the picker of the badge of `command`, with an empty search, or, for `None`, closes the one open.
  pub(super) fn set_picking(&self, command: Option<&str>) {
    let mut held = self.argument.get_untracked();
    let mut changed = false;
    for badge in &mut held {
      let picking = Some(badge.command) == command;
      changed |= badge.picking != picking;
      badge.picking = picking;
    }
    if changed {
      if command.is_some() {
        self.clear_search();
      }
      self.argument.set(held);
    }
  }

  /// The picker closes by its keys, and the composer's field takes the focus back.
  fn close_picker(&self) {
    self.refocus.store(true, Ordering::Relaxed);
    self.set_picking(None);
  }

  /// Removes the badge of `command`, with its choices.
  pub(super) fn remove_badge(&self, command: &str) {
    let mut left = self.argument.get_untracked();
    left.retain(|badge| badge.command != command);
    self.keep(left);
  }

  /// The badges become `left`; what the others did, and a picker that no longer shows, go.
  fn keep(&self, left: Vec<Arguing>) {
    self
      .taking
      .lock()
      .retain(|(held, _)| left.iter().any(|badge| badge.command == *held));
    if !left.iter().any(|badge| badge.picking) {
      self.forget_choices();
    }
    if self.argument.get_untracked() != left {
      self.argument.set(left);
    }
  }

  /// Enter or Shift Enter in the picker: its focused row, while it can be chosen and its badge does not hold it yet,
  /// becomes the badge's last choice, and the search empties. Whether a row was focused that can be chosen.
  fn add_focused(&self) -> bool {
    let offered = self.offered_choices.lock().clone().unwrap_or_default();
    let Some((choice, true)) = offered.focused.and_then(|at| offered.rows.get(at)).cloned() else {
      return false;
    };
    let mut held = self.argument.get_untracked();
    let Some(badge) = held.iter_mut().find(|badge| badge.picking) else {
      return false;
    };
    if !badge.choices.contains(&choice) {
      badge.choices.push(choice);
      self.argument.set(held);
    }
    self.clear_search();
    true
  }

  fn clear_search(&self) {
    if !self.search.get_untracked().is_empty() {
      self.search.set(String::new());
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

  /// Backspace in an empty field: the open picker's badge loses its last choice, and goes once it has none; with no
  /// picker open, the last badge goes.
  fn remove_last(&self) {
    let mut held = self.argument.get_untracked();
    let Some(at) = held
      .iter()
      .position(|badge| badge.picking)
      .or_else(|| held.len().checked_sub(1))
    else {
      return;
    };
    if held[at].picking && held[at].choices.pop().is_some() {
      self.argument.set(held);
      return;
    }
    let command = held[at].command;
    self.remove_badge(command);
  }

  /// The picker no longer shows: the keys have no row of it to move to or add.
  pub(super) fn forget_choices(&self) {
    *self.offered_choices.lock() = None;
  }

  /// The composer's field takes the focus back, once, after the picker closed by its keys.
  pub(super) fn take_focus_back(&self, ctx: &Ctx) {
    if self.refocus.swap(false, Ordering::Relaxed)
      && let Some(input) = &self.input
    {
      ctx.focus(input);
    }
  }
}

/// A key pressed with no modifier.
fn plain(event: &KeyboardEvent) -> bool {
  !event.shift && !event.ctrl && !event.meta && !event.alt
}
