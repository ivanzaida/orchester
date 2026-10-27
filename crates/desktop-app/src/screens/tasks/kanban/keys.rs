//! The board's keys (`Hint` `I9RqiS` in `07 Card actions and keys` `pXzH8`): the arrows move the focus from card to
//! card, Enter opens the focused card's task (or answers what is asked in place), `[` and `]` move it a column left or
//! right as a drop does, X selects it, `.` opens its menu and M, P and T the menu's Move to, Priority and Tags, Ctrl+C
//! (Cmd+C on macOS) copies its key, and Esc sets a lifted card down, keeps what is asked, or clears the selection.
//! None clashes with the window's shortcuts (`navigation::shortcuts`: Alt+←/→ and Cmd+[ / Cmd+] are Back and
//! Forward, Ctrl+T a new tab): the board's take no modifier but Ctrl+C / Cmd+C, and step aside while a text field has
//! the keys. A read-only card (`Dropping::locked`) is opened and its key copied, but the keys neither move nor select
//! it, nor open its menu's lists.

use std::sync::Arc;

use lurq::app::events::KeyboardEvent;
use orchester_core::{id::TaskId, project::TaskKey};

use super::{
  actions::{Focused, MenuPage, OpenMenu, Selected, Target},
  ask::answer,
  card::CardModel,
  drop::Dropping,
  model::ColumnKey,
};
use crate::{
  navigation::{Navigation, press::OpenIn, shortcuts::Keys},
  routes::{ProjectPage, Route, TaskView},
  ui::clipboard::Clipboard,
};

/// The board's cards by column, in order, as the keys move through them.
pub type Grid = Arc<Vec<(ColumnKey, Vec<CardModel>)>>;

/// What the keys work with.
#[derive(Clone)]
pub struct KeyContext {
  pub dropping: Dropping,
  pub grid: Grid,
  pub navigation: Navigation,
  pub clipboard: Clipboard,
}

/// Handles `event` on the board, unless a text field has the keys or a modifier other than the copy's is held.
pub fn on_key(keys: &KeyContext, event: &KeyboardEvent) {
  if event.default_prevented() || event.text_input_focused || event.alt {
    return;
  }
  let typed = Keys::of(event);
  if typed.only_command() && typed.is("c", "KeyC") {
    if let Some(card) = focused(keys) {
      event.prevent_default();
      if !keys.clipboard.copy(&card.row.key) {
        tracing::warn!("the task's key was not copied");
      }
    }
    return;
  }
  if event.ctrl || event.meta {
    return;
  }
  let handled = match event.key.as_str() {
    "Escape" => escape(keys),
    "Enter" => enter(keys),
    "ArrowUp" => step(keys, (0, -1)),
    "ArrowDown" => step(keys, (0, 1)),
    "ArrowLeft" => step(keys, (-1, 0)),
    "ArrowRight" => step(keys, (1, 0)),
    "[" => shift(keys, -1),
    "]" => shift(keys, 1),
    "x" | "X" => select(keys),
    "." => menu(keys, MenuPage::Commands),
    "m" | "M" => menu(keys, MenuPage::MoveTo),
    "p" | "P" => menu(keys, MenuPage::Priority),
    "t" | "T" => menu(keys, MenuPage::Tags),
    _ => false,
  };
  if handled {
    event.prevent_default();
  }
}

fn focused(keys: &KeyContext) -> Option<CardModel> {
  let id = keys.dropping.state.focused.get_untracked().0?;
  keys
    .grid
    .iter()
    .flat_map(|(_, cards)| cards)
    .find(|card| card.row.id == id)
    .cloned()
}

/// Where the focused card is: its column's index and its own.
fn place(keys: &KeyContext, id: TaskId) -> Option<(usize, usize)> {
  keys
    .grid
    .iter()
    .enumerate()
    .find_map(|(column, (_, cards))| Some((column, cards.iter().position(|card| card.row.id == id)?)))
}

fn escape(keys: &KeyContext) -> bool {
  let state = &keys.dropping.state;
  if state.drag.get_untracked().is_some() {
    *state.set_down.lock() = true;
    state.drag.set(None);
    return true;
  }
  if state.asked.get_untracked().is_some() {
    state.asked.set(None);
    return true;
  }
  if !state.selected.get_untracked().0.is_empty() {
    state.selected.set(Selected::default());
    return true;
  }
  if state.refused.get_untracked().is_some() {
    state.refused.set(None);
    return true;
  }
  false
}

fn enter(keys: &KeyContext) -> bool {
  let state = &keys.dropping.state;
  if let Some(asked) = state.asked.get_untracked() {
    answer(state, &keys.dropping.closer, &asked);
    return true;
  }
  let Some(card) = focused(keys) else {
    return false;
  };
  let Ok(task) = card.row.key.parse::<TaskKey>() else {
    return false;
  };
  let page = if card.row.not_shared {
    ProjectPage::KeptTask(task, TaskView::default())
  } else {
    ProjectPage::Task(task, TaskView::default())
  };
  keys
    .navigation
    .open(Route::Project(card.project, page), OpenIn::ActiveTab);
  true
}

/// Moves the focus by `(columns, cards)`; the first card takes it when none has it.
fn step(keys: &KeyContext, (across, down): (isize, isize)) -> bool {
  let state = &keys.dropping.state;
  let at = state.focused.get_untracked().0.and_then(|id| place(keys, id));
  let next = match at {
    None => keys
      .grid
      .iter()
      .find_map(|(_, cards)| cards.first())
      .map(|card| card.row.id),
    Some((column, card)) => {
      let column = column
        .saturating_add_signed(across)
        .min(keys.grid.len().saturating_sub(1));
      let cards = &keys.grid[column].1;
      let card = if across == 0 {
        card.saturating_add_signed(down)
      } else {
        card
      };
      cards
        .get(card.min(cards.len().saturating_sub(1)))
        .map(|card| card.row.id)
    }
  };
  match next {
    Some(id) => {
      state.focused.set(Focused(Some(id)));
      true
    }
    None => false,
  }
}

/// The focused card, unless it is read-only (`Dropping::locked`).
fn changeable(keys: &KeyContext) -> Option<CardModel> {
  focused(keys).filter(|card| !keys.dropping.locked(card))
}

/// Moves the focused card a column left (`-1`) or right (`1`), as a drop there does; a read-only card stays.
fn shift(keys: &KeyContext, by: isize) -> bool {
  let Some(card) = changeable(keys) else {
    return false;
  };
  let Some((column, _)) = place(keys, card.row.id) else {
    return false;
  };
  let Some(to) = column.checked_add_signed(by).and_then(|to| keys.grid.get(to)) else {
    return false;
  };
  let target = Target {
    column: to.0.clone(),
    lane: None,
  };
  keys.dropping.drop_card(&card, &target);
  true
}

/// Selects the focused card, or no longer; a read-only card is never selected.
fn select(keys: &KeyContext) -> bool {
  let Some(card) = changeable(keys) else {
    return false;
  };
  let id = card.row.id;
  keys.dropping.state.selected.update(|selected| {
    if !selected.0.remove(&id) {
      selected.0.insert(id);
    }
  });
  true
}

/// Opens the focused card's menu at `page`: a *Not shared* card has none, and a read-only card's has no lists to
/// change it with.
fn menu(keys: &KeyContext, page: MenuPage) -> bool {
  let Some(card) = focused(keys).filter(|card| !card.row.not_shared) else {
    return false;
  };
  if page != MenuPage::Commands && keys.dropping.locked(&card) {
    return false;
  }
  keys.dropping.state.menu.set(OpenMenu(Some((card.row.id, page))));
  true
}
