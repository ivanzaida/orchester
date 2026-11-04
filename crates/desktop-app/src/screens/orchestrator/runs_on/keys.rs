//! The runs-on picker's keys (`Notes` `JbhvF` in `t7cdi`): ← and → move between the rail, Models and Effort, ↑ and ↓
//! within one, Enter chooses where the keys are; a key typed while the focus is elsewhere in the picker goes to Search.
//! Esc closes the picker (its popup's own). lurq hands a key to every node that takes keys, from the window's root
//! down: the picker's are its own only while it is open, and it marks each it took.

use lurq::app::events::KeyboardEvent;

use super::{
  models::ModelItem,
  picking::{Chooser, Lists, Part},
};

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
  picking.part.set(next);
  picking.at.set(Some(lists.home(next)));
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
  true
}

/// Chooses where the keys are; nothing while they are nowhere.
fn choose(chooser: &Chooser, lists: &Lists) -> bool {
  let part = chooser.picking.part.get_untracked();
  let searched = part == Part::Models && !chooser.picking.query.get_untracked().trim().is_empty();
  let Some(at) = chooser.picking.at.get_untracked().or(searched.then_some(0)) else {
    return false;
  };
  match part {
    Part::Rail => {
      if let Some((connection, true)) = lists.rail.get(at) {
        chooser.connection(*connection);
      }
    }
    Part::Models => match lists.models.get(at) {
      Some(ModelItem::Listed(model) | ModelItem::Typed(model)) => chooser.model(model.clone()),
      Some(ModelItem::ShowAll(_)) => chooser.show_all(),
      None => {}
    },
    Part::Effort => {
      if let Some(effort) = lists.efforts.get(at) {
        chooser.effort(effort.clone());
      }
    }
  }
  true
}
