//! The tab list's keyboard (`a1i6Ws`, `qWCBQ`): Ctrl+Shift+A (Cmd+Shift+A) opens and closes it. While it
//! is open, ArrowUp and ArrowDown move the ring over the rows it lists (Home and End to their ends, unless
//! the search field has the keys), Enter or Space switches to the ringed tab, Ctrl+W (Cmd+W) closes the
//! ringed tab, and Esc clears the search or, with none, closes the list. While it is open the list holds
//! the window's first keys (`navigation::input::FirstKeys`), so it takes them before the page and the
//! window's own shortcuts.

use std::sync::Arc;

use lurq::{app::events::KeyboardEvent, core::Signal};
use parking_lot::Mutex;

use crate::navigation::{
  Navigation,
  shortcuts::{Keys, Shortcut, opens_tab_list, shortcut},
  tabs::TabId,
};

/// The keyboard's row, for the search it was chosen in: a new search rings its first match again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ring {
  pub tab: TabId,
  pub query: String,
}

impl lurq::app::component::DevtoolsInspectable for Ring {}

/// The rows the open list shows, as it last showed them.
#[derive(Clone, Debug, Default)]
pub struct Listing {
  pub rows: Vec<TabId>,
  pub ringed: Option<TabId>,
  pub current: Option<TabId>,
}

/// Where the list goes after a key.
pub enum Outcome {
  /// Nothing the list does.
  Ignored,
  Ring(TabId),
  Choose(TabId),
  Close(TabId),
  ClearSearch,
  /// Closes the list; the focus goes back to its button.
  Dismiss,
  /// Opens or closes the list from the keyboard.
  Toggle,
}

/// What `event` does to a list that is `open`, showing `listing`, searched for `query`.
pub fn outcome(event: &KeyboardEvent, open: bool, listing: &Listing, query: &str) -> Outcome {
  let keys = Keys::of(event);
  if opens_tab_list(keys) {
    return Outcome::Toggle;
  }
  if !open {
    return Outcome::Ignored;
  }
  if shortcut(keys) == Some(Shortcut::CloseTab) {
    return listing.ringed.map_or(Outcome::Ignored, Outcome::Close);
  }
  if event.ctrl || event.alt || event.meta || event.shift {
    return Outcome::Ignored;
  }
  let typing = event.text_input_focused;
  match event.key.as_str() {
    "ArrowDown" => step(listing, 1),
    "ArrowUp" => step(listing, -1),
    "Home" if !typing => ends(listing, false),
    "End" if !typing => ends(listing, true),
    "Enter" => listing.ringed.map_or(Outcome::Dismiss, Outcome::Choose),
    " " if !typing => listing.ringed.map_or(Outcome::Dismiss, Outcome::Choose),
    "Escape" if !query.is_empty() => Outcome::ClearSearch,
    "Escape" => Outcome::Dismiss,
    _ => Outcome::Ignored,
  }
}

/// The ring one row down (`by` 1) or up (-1), stopping at the ends; with no ring, on the current row, or
/// the first one when it is not listed.
fn step(listing: &Listing, by: isize) -> Outcome {
  let rows = &listing.rows;
  let Some(first) = rows.first() else {
    return Outcome::Ignored;
  };
  let Some(at) = listing
    .ringed
    .and_then(|ringed| rows.iter().position(|row| *row == ringed))
  else {
    let current = listing.current.filter(|current| rows.contains(current));
    return Outcome::Ring(current.unwrap_or(*first));
  };
  let to = at.saturating_add_signed(by).min(rows.len() - 1);
  Outcome::Ring(rows[to])
}

fn ends(listing: &Listing, last: bool) -> Outcome {
  let row = if last {
    listing.rows.last()
  } else {
    listing.rows.first()
  };
  row.map_or(Outcome::Ignored, |row| Outcome::Ring(*row))
}

/// The list's state the keys change.
#[derive(Clone)]
pub struct ListKeys {
  pub navigation: Navigation,
  pub open: Signal<bool>,
  pub query: Signal<String>,
  pub ring: Signal<Option<Ring>>,
  pub listing: Arc<Mutex<Listing>>,
  /// Opens the list, or closes it, as its button does.
  pub toggle: Arc<dyn Fn() + Send + Sync>,
  /// Closes the list, the focus back on its button.
  pub dismiss: Arc<dyn Fn() + Send + Sync>,
}

impl ListKeys {
  /// The handler of the list's keys.
  pub fn handler(self) -> impl Fn(KeyboardEvent) + Send + Sync + 'static {
    move |event: KeyboardEvent| {
      if event.default_prevented() {
        return;
      }
      let query = self.query.get_untracked();
      let listing = self.listing.lock().clone();
      match outcome(&event, self.open.get_untracked(), &listing, &query) {
        Outcome::Ignored => return,
        Outcome::Ring(tab) => self.ring.set(Some(Ring { tab, query })),
        Outcome::Choose(tab) => {
          self.navigation.select(tab);
          (self.dismiss)();
        }
        Outcome::Close(tab) => self.navigation.close(tab),
        Outcome::ClearSearch => self.query.set(String::new()),
        Outcome::Dismiss => (self.dismiss)(),
        Outcome::Toggle => (self.toggle)(),
      }
      event.prevent_default();
    }
  }
}
