//! The runs-on picker's keys (`Notes` `JbhvF` in `t7cdi`): ← and → move between the rail, Models and Effort, ↑ and ↓
//! within one (the models' list scrolls to keep the place in view, and on an API provider ↓ reaches the typed effort's
//! field), Enter chooses where the keys are; a key typed while the focus is elsewhere in the picker goes to Search.
//! Enter on a connection that cannot be chosen says why. Esc closes the picker (its popup's own). lurq hands a key to
//! every node that takes keys, from the window's root down: the picker's are its own only while it is open, and it
//! marks each it took.

use lurq::app::events::KeyboardEvent;
use orchester_core::id::ConnectionId;

use super::{
  models::{self, ModelItem},
  picking::{Chooser, Part},
};

/// A place in the Effort column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum EffortItem {
  /// *Default*: the CLI's own.
  Default,
  /// An effort the connection lists.
  Listed(String),
  /// The field an API provider's effort is typed in.
  Typed,
}

/// What the keys move over: the rail's connections (whether each can be chosen), the models shown and the efforts;
/// and in each, where the one shown or chosen is.
pub(super) struct Lists {
  pub(super) rail: Vec<(ConnectionId, bool)>,
  pub(super) models: Vec<ModelItem>,
  pub(super) efforts: Vec<EffortItem>,
  pub(super) homes: [Option<usize>; 3],
}

impl Lists {
  pub(super) fn len(&self, part: Part) -> usize {
    match part {
      Part::Rail => self.rail.len(),
      Part::Models => self.models.len(),
      Part::Effort => self.efforts.len(),
    }
  }

  /// Where the keys start in `part`: on the one shown or chosen, else the first.
  pub(super) fn home(&self, part: Part) -> usize {
    let index = match part {
      Part::Rail => 0,
      Part::Models => 1,
      Part::Effort => 2,
    };
    self.homes[index].unwrap_or(0)
  }

  /// The columns the keys can be in, in their order: the rail and Effort only when they show something.
  fn parts(&self) -> Vec<Part> {
    [Part::Rail, Part::Models, Part::Effort]
      .into_iter()
      .filter(|part| *part == Part::Models || self.len(*part) > 0)
      .collect()
  }

  /// The keys at `at` in `part` are on the typed effort's field.
  fn on_field(&self, (part, at): (Part, usize)) -> bool {
    part == Part::Effort && self.efforts.get(at) == Some(&EffortItem::Typed)
  }
}

/// What a key does in the picker over `lists`.
pub(super) fn keys(chooser: &Chooser, lists: Lists) -> impl Fn(KeyboardEvent) + Send + Sync + 'static + use<> {
  let chooser = chooser.clone();
  move |event: KeyboardEvent| {
    if event.default_prevented() || event.composing {
      return;
    }
    let took = match event.key.as_str() {
      "ArrowLeft" => switch(&chooser, &lists, -1),
      "ArrowRight" => switch(&chooser, &lists, 1),
      "ArrowUp" => step(&chooser, &lists, -1),
      "ArrowDown" => step(&chooser, &lists, 1),
      "Enter" => choose(&chooser, &lists),
      typed if typed.chars().count() == 1 && !(event.ctrl || event.alt || event.meta || event.text_input_focused) => {
        chooser.picking.type_in(typed);
        true
      }
      _ => false,
    };
    if took {
      event.prevent_default();
    }
  }
}

/// Moves the keys `by` columns, to the one shown or chosen there.
fn switch(chooser: &Chooser, lists: &Lists, by: isize) -> bool {
  let picking = &chooser.picking;
  let parts = lists.parts();
  let at = parts
    .iter()
    .position(|part| *part == picking.part.get_untracked())
    .unwrap_or(0);
  let Some(next) = at.checked_add_signed(by).and_then(|next| parts.get(next).copied()) else {
    return true;
  };
  let home = lists.home(next);
  picking.part.set(next);
  picking.at.set(Some(home));
  picking.focus_for(lists.on_field((next, home)));
  if next == Part::Models {
    models::reveal(&picking.scroll, home);
  }
  true
}

/// Moves the keys `by` places in their column; the first press shows where they are.
fn step(chooser: &Chooser, lists: &Lists, by: isize) -> bool {
  let picking = &chooser.picking;
  let part = picking.part.get_untracked();
  let count = lists.len(part);
  if count == 0 {
    return true;
  }
  let searched = part == Part::Models && !picking.query.get_untracked().trim().is_empty();
  let next = match picking.at.get_untracked() {
    Some(at) => at.saturating_add_signed(by).min(count - 1),
    None if searched => 0_usize.saturating_add_signed(by).min(count - 1),
    None => lists.home(part),
  };
  picking.at.set(Some(next));
  picking.focus_for(lists.on_field((part, next)));
  if part == Part::Models {
    models::reveal(&picking.scroll, next);
  }
  true
}

/// Chooses where the keys are; the typed effort while its field has the focus; nothing while they are nowhere.
fn choose(chooser: &Chooser, lists: &Lists) -> bool {
  let picking = &chooser.picking;
  if picking.effort_field.focused() {
    chooser.typed_effort();
    return true;
  }
  let part = picking.part.get_untracked();
  let searched = part == Part::Models && !picking.query.get_untracked().trim().is_empty();
  let Some(at) = picking.at.get_untracked().or(searched.then_some(0)) else {
    return false;
  };
  match part {
    Part::Rail => match lists.rail.get(at) {
      Some((connection, true)) => chooser.connection(*connection),
      Some((connection, false)) => picking.refuse(*connection),
      None => {}
    },
    Part::Models => {
      if let Some(ModelItem::Listed(model) | ModelItem::Typed(model)) = lists.models.get(at) {
        chooser.model(model.clone());
      }
    }
    Part::Effort => match lists.efforts.get(at) {
      Some(EffortItem::Default) => chooser.effort(None),
      Some(EffortItem::Listed(effort)) => chooser.effort(Some(effort.clone())),
      Some(EffortItem::Typed) => chooser.typed_effort(),
      None => {}
    },
  }
  true
}
