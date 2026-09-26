//! What history keeps beside the path of an entry, as the router's state of that entry: the
//! path the entry was opened from, and whether it holds a step the user finished (a submitted
//! form). The router shows only the current entry, so knowing where an entry came from is how
//! leaving a finished step can return to the entry before it instead of repeating it.

use std::sync::Arc;

use lurq::router::Navigator;

use super::route::Route;

#[derive(Clone, Debug, Default)]
struct Entry {
  from: Option<Arc<str>>,
  finished: bool,
}

/// Opens `path` as a new entry that remembers the current one.
pub fn push(navigator: &Navigator, path: String) {
  let from = navigator.path().get_untracked();
  navigator.push_with_state(
    path,
    Entry {
      from: Some(from.into()),
      finished: false,
    },
  );
}

/// Shows `path` in place of the current entry, which still follows the same entry.
pub fn replace(navigator: &Navigator, path: String) {
  let from = current(navigator).from;
  navigator.replace_with_state(path, Entry { from, finished: false });
}

/// The route of the entry the current one was opened from, when it is known.
pub fn from(navigator: &Navigator) -> Option<Route> {
  Route::parse(&current(navigator).from?)
}

/// The current entry holds a step the user finished, and the entry it was opened from is where
/// the step leads: history goes back to that entry, so neither the step nor a repeat of that
/// entry stays in history, and going forward skips the finished step.
pub fn finish_back(navigator: &Navigator) {
  let path = navigator.path().get_untracked();
  let from = current(navigator).from;
  navigator.replace_with_state(path, Entry { from, finished: true });
  navigator.back();
}

/// Whether the current entry holds a finished step.
pub fn is_finished(navigator: &Navigator) -> bool {
  current(navigator).finished
}

/// The state of the current entry; an entry without one (the start screen, a redirect) came
/// from nowhere known.
fn current(navigator: &Navigator) -> Entry {
  navigator
    .state::<Entry>()
    .map_or_else(Entry::default, |entry| Entry::clone(&entry))
}
