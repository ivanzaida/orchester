//! *More actions* (`JapSp`) and the runs' menus as the tests read them: a row that can be chosen says its label alone;
//! one that cannot ends in a lock with why, or says it is under way.

use lurq::node::element::ElementRef;
use orchester_app::ui::icon::Icon;

use crate::support::Window;

/// Opens the header's *More actions*, unless it is open.
pub fn open(window: &mut Window) {
  if window.tree.get_element_by_id("task-more-panel").is_none() {
    window.press("task-more");
  }
  window.until("More actions", |window| window.texts("task-more-panel").is_some());
}

/// Opens *More actions* and chooses `id` once it can be chosen.
pub fn choose(window: &mut Window, id: &str) {
  window.until(id, |window| ready_after_opening(window, id));
  window.press(id);
}

/// Whether the row `id` can be chosen, *More actions* opened first while it is closed: for a wait on a run's state.
pub fn ready_after_opening(window: &mut Window, id: &str) -> bool {
  if window.tree.get_element_by_id("task-more-panel").is_none() {
    window.press("task-more");
  }
  ready(window, id)
}

/// Whether the menu row `id` shows and can be chosen: its label alone, no lock and no reason.
pub fn ready(window: &mut Window, id: &str) -> bool {
  window.texts(id).is_some_and(|texts| texts.len() == 1) && !locked(window, id)
}

/// Whether the menu row `id` ends in the lock of an unavailable action.
pub fn locked(window: &mut Window, id: &str) -> bool {
  let lock = Icon::Lock.glyph().to_string();
  window.tree.get_element_by_id(id).is_some_and(|row| holds(row, &lock))
}

fn holds(element: ElementRef<'_>, text: &str) -> bool {
  if element.text_content() == Some(text) {
    return true;
  }
  for child in element.children() {
    if holds(child, text) {
      return true;
    }
  }
  false
}
