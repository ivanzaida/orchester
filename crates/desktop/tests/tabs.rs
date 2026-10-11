//! The window's tabs as data (design doc, "Tabs"): opening right of the active tab, closing and
//! what becomes active, the last tab leaving a fresh All projects tab, reordering, what is saved
//! and restored, the strip's widths and overflow, and the shortcuts.

extern crate lurq_legacy as lurq;

use lurq::{app::events::KeyboardEvent, core::NodeId};
use orchester_desktop::tabs::{
  input::{Shortcut, shortcut},
  model::{Closed, Focus, SavedTabs, TabSet},
  strip::layout::{ACTIVE_MIN_WIDTH, CLOSE_MIN_WIDTH, StripLayout, TAB_MAX_WIDTH, TAB_MIN_WIDTH},
};

const HOME: &str = "/projects";

fn paths(set: &TabSet) -> Vec<String> {
  set.tabs().iter().map(|tab| tab.path().to_string()).collect()
}

fn active_path(set: &TabSet) -> String {
  set.active_tab().path().to_string()
}

/// Tabs at `a`, `b` and `c`, `a` active.
fn three() -> TabSet {
  let mut set = TabSet::new(HOME, "/a");
  set.open("/c", Focus::Background);
  set.open("/b", Focus::Background);
  set
}

#[test]
fn tabs_open_right_of_the_active_one() {
  let set = three();
  assert_eq!(paths(&set), ["/a", "/b", "/c"], "each opens right of the active tab");
  assert_eq!(active_path(&set), "/a", "background tabs do not switch");

  let mut set = set;
  set.select(1);
  let id = set.open_home();
  assert_eq!(paths(&set), ["/a", "/b", HOME, "/c"]);
  assert_eq!(set.active(), id, "a new tab is active");
}

#[test]
fn tabs_closing_the_active_one_activates_the_next_or_the_previous() {
  let mut set = three();
  set.select(1);
  let b = set.active();
  assert_eq!(set.close(b), Closed::Closed);
  assert_eq!(paths(&set), ["/a", "/c"]);
  assert_eq!(active_path(&set), "/c", "the tab to the right becomes active");
  let c = set.active();
  assert_eq!(set.close(c), Closed::Closed);
  assert_eq!(active_path(&set), "/a", "the last one gives way to the one left of it");
}

#[test]
fn tabs_closing_an_inactive_one_keeps_the_active_tab() {
  let mut set = three();
  let c = set.tabs()[2].id();
  set.close(c);
  assert_eq!(active_path(&set), "/a");
  assert_eq!(paths(&set), ["/a", "/b"]);
}

#[test]
fn tabs_closing_the_last_one_opens_all_projects_in_a_fresh_tab() {
  let mut set = TabSet::new(HOME, "/project/1/tasks");
  let only = set.active();
  assert!(set.closable(only));
  assert_eq!(set.close(only), Closed::Replaced);
  assert_eq!(paths(&set), [HOME]);
  assert_ne!(set.active(), only, "the fresh tab has its own identity and history");
  let home = set.active();
  assert!(!set.closable(home), "a sole All projects tab has no close button");
  assert_eq!(set.close(home), Closed::Kept);
  assert_eq!(set.active(), home);
}

#[test]
fn tabs_next_and_previous_wrap_and_numbers_select() {
  let mut set = three();
  set.activate_previous();
  assert_eq!(active_path(&set), "/c", "previous wraps to the last tab");
  set.activate_next();
  assert_eq!(active_path(&set), "/a", "next wraps to the first tab");
  set.select(1);
  assert_eq!(active_path(&set), "/b");
  assert!(!set.select(7), "there is no eighth tab");
  assert_eq!(active_path(&set), "/b");
  set.select_last();
  assert_eq!(active_path(&set), "/c");
}

#[test]
fn tabs_reorder_keeps_the_active_tab() {
  let mut set = three();
  let a = set.active();
  assert!(set.move_tab(a, 2));
  assert_eq!(paths(&set), ["/b", "/c", "/a"]);
  assert_eq!(set.active(), a);
  assert!(!set.move_tab(a, 99), "past the end is the end, where it already is");
  assert_eq!(paths(&set), ["/b", "/c", "/a"]);
  assert!(!set.move_tab(a, 2), "moving to where it is changes nothing");
}

#[test]
fn tabs_save_and_restore_their_order_locations_and_the_active_one() {
  let mut set = three();
  set.select(2);
  let b = set.tabs()[1].id();
  assert!(set.set_path(b, "/b?tab=drafts"));
  assert!(!set.set_path(b, "/b?tab=drafts"), "the same path is no change");
  let saved = set.saved();
  assert_eq!(saved.active, 2);
  let text = saved.encode();
  let decoded = SavedTabs::decode(&text).expect("decodes what it encoded");
  assert_eq!(decoded, saved);
  let restored = TabSet::restored(HOME, &decoded).expect("tabs");
  assert_eq!(paths(&restored), ["/a", "/b?tab=drafts", "/c"]);
  assert_eq!(active_path(&restored), "/c");
}

#[test]
fn tabs_saved_text_that_cannot_be_read_is_ignored() {
  assert_eq!(SavedTabs::decode(""), None);
  assert_eq!(SavedTabs::decode("something else\nactive 0\n/a\n"), None);
  assert_eq!(SavedTabs::decode("orchester-tabs 1\nactive x\n/a\n"), None);
  assert_eq!(SavedTabs::decode("orchester-tabs 1\nactive 0\n"), None, "no tab");
  assert_eq!(SavedTabs::decode("orchester-tabs 1\nactive 0\nnot a path\n"), None);
  let out_of_range = SavedTabs::decode("orchester-tabs 1\nactive 5\n/a\n/b\n").expect("tabs");
  assert_eq!(out_of_range.active, 0, "an active tab that is not there is the first");
}

#[test]
fn tabs_strip_shrinks_then_scrolls() {
  let wide = StripLayout::new(3, 0, 1006.0, 56.0);
  assert!(wide.tabs.iter().all(|(_, width)| *width == TAB_MAX_WIDTH));
  assert!(!wide.overflow);
  assert!(wide.inactive_close);

  let shrunk = StripLayout::new(8, 3, 1006.0, 56.0);
  assert!(!shrunk.overflow);
  assert!(
    shrunk.tabs[3].1 >= ACTIVE_MIN_WIDTH,
    "the active tab keeps its least width"
  );
  let inactive = shrunk.tabs[0].1;
  assert!((TAB_MIN_WIDTH..CLOSE_MIN_WIDTH).contains(&inactive), "{inactive}");
  assert!(!shrunk.inactive_close, "narrow inactive tabs drop their close button");
  assert!((shrunk.content - 1006.0).abs() < 0.5, "the tabs fill the room");

  let overflow = StripLayout::new(14, 13, 1006.0, 56.0);
  assert!(overflow.overflow);
  assert_eq!(overflow.tabs[0].1, TAB_MIN_WIDTH);
  assert_eq!(overflow.tabs[13].1, ACTIVE_MIN_WIDTH);
  assert!(
    (overflow.viewport - 950.0).abs() < 0.5,
    "the tab list button takes its room"
  );
  let scroll = overflow.scroll_to(13, 0.0);
  assert!(
    (scroll - (overflow.content - overflow.viewport)).abs() < 0.5,
    "the last tab scrolls into view"
  );
  assert_eq!(overflow.scroll_to(0, scroll), 0.0, "and so does the first");
  assert_eq!(overflow.scroll_to(5, 200.0), 200.0, "a tab in view does not scroll");
}

#[test]
fn tabs_strip_drops_a_dragged_tab_where_its_middle_is() {
  let layout = StripLayout::new(4, 0, 1006.0, 56.0);
  assert_eq!(layout.drop_index(0, 0.0), 0);
  assert_eq!(layout.drop_index(0, 100.0), 0, "short of the next tab's middle");
  assert_eq!(layout.drop_index(0, 250.0), 1, "past the next tab's middle");
  assert_eq!(layout.drop_index(0, 700.0), 3);
  assert_eq!(layout.drop_index(3, 0.0), 0);
  let three = StripLayout::new(3, 0, 1006.0, 56.0);
  assert_eq!(three.drop_index(0, 448.0), 2, "pushed against the end, it lands last");
}

fn key(key: &str, code: &str, (ctrl, shift, alt, meta): (bool, bool, bool, bool)) -> KeyboardEvent {
  KeyboardEvent::new(key, code, shift, ctrl, alt, meta, NodeId::UNASSIGNED)
}

#[cfg(not(target_os = "macos"))]
#[test]
fn tabs_shortcuts_on_windows() {
  let ctrl = (true, false, false, false);
  let ctrl_shift = (true, true, false, false);
  assert_eq!(shortcut(&key("t", "KeyT", ctrl)), Some(Shortcut::NewTab));
  assert_eq!(shortcut(&key("w", "KeyW", ctrl)), Some(Shortcut::CloseTab));
  assert_eq!(shortcut(&key("Tab", "Tab", ctrl)), Some(Shortcut::NextTab));
  assert_eq!(shortcut(&key("Tab", "Tab", ctrl_shift)), Some(Shortcut::PreviousTab));
  assert_eq!(shortcut(&key("PageDown", "PageDown", ctrl)), Some(Shortcut::NextTab));
  assert_eq!(shortcut(&key("PageUp", "PageUp", ctrl)), Some(Shortcut::PreviousTab));
  assert_eq!(shortcut(&key("1", "Digit1", ctrl)), Some(Shortcut::SelectTab(0)));
  assert_eq!(shortcut(&key("8", "Digit8", ctrl)), Some(Shortcut::SelectTab(7)));
  assert_eq!(shortcut(&key("9", "Digit9", ctrl)), Some(Shortcut::LastTab));
  let alt = (false, false, true, false);
  assert_eq!(shortcut(&key("ArrowLeft", "ArrowLeft", alt)), Some(Shortcut::Back));
  assert_eq!(shortcut(&key("ArrowRight", "ArrowRight", alt)), Some(Shortcut::Forward));
  assert_eq!(shortcut(&key("t", "KeyT", (false, false, false, false))), None);
  assert_eq!(shortcut(&key("0", "Digit0", ctrl)), None);
}

#[cfg(target_os = "macos")]
#[test]
fn tabs_shortcuts_on_macos() {
  let cmd = (false, false, false, true);
  let cmd_shift = (false, true, false, true);
  let ctrl = (true, false, false, false);
  assert_eq!(shortcut(&key("t", "KeyT", cmd)), Some(Shortcut::NewTab));
  assert_eq!(shortcut(&key("w", "KeyW", cmd)), Some(Shortcut::CloseTab));
  assert_eq!(
    shortcut(&key("Tab", "Tab", ctrl)),
    Some(Shortcut::NextTab),
    "Ctrl+Tab on macOS too"
  );
  assert_eq!(shortcut(&key("]", "BracketRight", cmd_shift)), Some(Shortcut::NextTab));
  assert_eq!(
    shortcut(&key("[", "BracketLeft", cmd_shift)),
    Some(Shortcut::PreviousTab)
  );
  assert_eq!(shortcut(&key("1", "Digit1", cmd)), Some(Shortcut::SelectTab(0)));
  assert_eq!(shortcut(&key("9", "Digit9", cmd)), Some(Shortcut::LastTab));
  assert_eq!(shortcut(&key("[", "BracketLeft", cmd)), Some(Shortcut::Back));
  assert_eq!(shortcut(&key("]", "BracketRight", cmd)), Some(Shortcut::Forward));
  assert_eq!(
    shortcut(&key("t", "KeyT", ctrl)),
    None,
    "Ctrl+T is not a macOS shortcut"
  );
}
