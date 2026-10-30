//! The window's tab strip headless (`support`): a tab's dot over its icon (`k7xvx`), and the strip's overflow (`01d`,
//! `01e`): the all-tabs menu's rows, the wheel, the fades and the tab shown.

use std::ops::Range;

use lurq::app::events::{MouseButton, ScrollPhase};
use orchester_ipc::browser::{BrowserEvent, BrowserMode, BrowserState, BrowserTabId, TabOwner};

use super::support::{Browser, browser_installed, state, tab};

/// `Icon · waits` `ORNFA` in `k7xvx`: the dot stands over the icon's corner, whole, and the icon's box stays the
/// icon's, so the label stands where it stands on a tab with no dot.
#[test]
fn browser_tab_dot_is_whole_over_the_icons_corner_and_leaves_the_label_alone() {
  if !browser_installed() {
    return;
  }
  let mut browser = Browser::open(false);
  browser.feed(state("P1", BrowserMode::Headless).0);
  browser.until_shown("browser-tab-0-dot");

  let (icon, dot) = (
    browser.bounds("browser-tab-0-icon"),
    browser.bounds("browser-tab-0-dot"),
  );
  assert_eq!((icon.width, icon.height), (13.0, 13.0), "the icon's box");
  assert_eq!(
    (dot.x - icon.x, dot.y - icon.y, dot.width, dot.height),
    (6.25, -2.75, 8.5, 8.5),
    "7 from the icon's left and 2 over its top, the 1.5 ring centred on the edge of its 7"
  );

  let (tab, label) = (browser.bounds("browser-tab-0"), browser.bounds("browser-tab-0-label"));
  let (plain, plain_label) = (browser.bounds("browser-tab-2"), browser.bounds("browser-tab-2-label"));
  assert_eq!(label.x - tab.x, 29.0, "`Q3022`: 10, the icon's 13 and 6");
  assert_eq!(
    plain_label.x - plain.x,
    29.0,
    "the same on the person's tab, with no dot"
  );

  let painted = browser.tree.painted_quads();
  let quad = painted
    .iter()
    .find(|quad| (quad.x, quad.y, quad.width, quad.height) == (dot.x, dot.y, dot.width, dot.height))
    .expect("the dot is painted");
  let clip = quad.clip;
  assert!(
    !clip.active
      || (clip.x <= quad.x
        && clip.y <= quad.y
        && quad.x + quad.width <= clip.x + clip.width
        && quad.y + quad.height <= clip.y + clip.height),
    "the dot {:?} lies inside its clip {:?}",
    (quad.x, quad.y, quad.width, quad.height),
    (clip.x, clip.y, clip.width, clip.height)
  );
}

#[test]
fn browser_overflow_menu_lists_every_tab_in_rows_with_a_close_under_the_pointer() {
  if !browser_installed() {
    return;
  }
  let mut browser = Browser::open(false);
  let tabs = (0..20)
    .map(|index| tab(&format!("P{index}"), TabOwner::Person, "Pull requests · GitHub"))
    .collect();
  browser.feed(BrowserEvent::Changed(BrowserState {
    tabs,
    shown: Some(BrowserTabId::new("P0")),
    mode: BrowserMode::Headless,
  }));
  browser.until_shown("browser-overflow");
  let center = |browser: &mut Browser, id: &str| {
    browser
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
      .center()
  };
  let (x, y) = center(&mut browser, "browser-overflow");
  browser.tree.mouse_down(x, y, MouseButton::Left);
  browser.tree.mouse_up(x, y, MouseButton::Left);
  browser.until_shown("browser-overflow-19");
  let row = |browser: &mut Browser, index: usize| {
    browser
      .tree
      .get_element_by_id_mut(&format!("browser-overflow-{index}"))
      .and_then(|element| element.bounds())
      .expect("the row is laid out")
  };
  let (first, second, last) = (row(&mut browser, 0), row(&mut browser, 1), row(&mut browser, 19));
  assert_eq!(
    (first.height, second.y - first.y),
    (28.0, 28.0),
    "rows of 28, one under another"
  );
  assert!(
    last.y + last.height <= 800.0,
    "all twenty rows show without scrolling, as 01d's"
  );
  assert!(!browser.shows("browser-overflow-1-close"), "× only under the pointer");
  let (x, y) = center(&mut browser, "browser-overflow-1");
  browser.tree.mouse_move(x, y);
  browser.until_shown("browser-overflow-1-close");
  assert!(!browser.shows("browser-overflow-2-close"));
}

#[test]
fn browser_tab_strip_scrolls_sideways_with_a_mouse_wheel() {
  if !browser_installed() {
    return;
  }
  let mut browser = Browser::open(false);
  let tabs = (0..20)
    .map(|index| tab(&format!("P{index}"), TabOwner::Person, "Pull requests · GitHub"))
    .collect();
  browser.feed(BrowserEvent::Changed(BrowserState {
    tabs,
    shown: Some(BrowserTabId::new("P0")),
    mode: BrowserMode::Headless,
  }));
  browser.until_shown("browser-overflow");
  let bounds = |browser: &mut Browser, id: &str| {
    browser
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  };
  let first = bounds(&mut browser, "browser-tab-0").x;
  assert!(
    !browser.shows("browser-tabs-fade-start"),
    "the strip starts at its first tab"
  );
  let (x, y) = bounds(&mut browser, "browser-tab-1").center();
  // A mouse wheel turned down, as lurq delivers it: a vertical delta only (`01e`, the strip scrolls with the wheel).
  browser.tree.scroll(x, y, 0.0, -600.0, ScrollPhase::Scroll);
  browser.pass();
  browser.pass();
  assert!(
    bounds(&mut browser, "browser-tab-0").x < first - 100.0,
    "the tabs moved to the left"
  );
  assert!(browser.shows("browser-tabs-fade-start"), "the cut start fades (01e)");
  browser.tree.scroll(x, y, 0.0, 2000.0, ScrollPhase::Scroll);
  browser.pass();
  browser.pass();
  assert_eq!(bounds(&mut browser, "browser-tab-0").x, first, "back at the first tab");
  assert!(!browser.shows("browser-tabs-fade-start"));
}

/// The person's tabs `P<numbers>`, `P15` shown, as a runner reports them.
fn with_p15_shown(numbers: Range<usize>) -> BrowserEvent {
  let tabs = numbers
    .map(|number| tab(&format!("P{number}"), TabOwner::Person, "Pull requests · GitHub"))
    .collect();
  BrowserEvent::Changed(BrowserState {
    tabs,
    shown: Some(BrowserTabId::new("P15")),
    mode: BrowserMode::Headless,
  })
}

/// Tab 15 of twenty is wholly between the fades of both cut edges (`01d`), and the overflow button counts the tabs not
/// wholly between the strip's start and itself.
fn assert_tab_15_in_view(browser: &mut Browser) {
  assert!(browser.shows("browser-tabs-fade-start"), "the cut start fades (01d)");
  assert!(browser.shows("browser-tabs-fade-end"), "the cut end fades (01d)");
  let (start, end, shown) = (
    browser.bounds("browser-tabs-fade-start"),
    browser.bounds("browser-tabs-fade-end"),
    browser.bounds("browser-tab-15"),
  );
  assert!(
    shown.x >= start.x + start.width && shown.x + shown.width <= end.x,
    "the tab shown is clear of the fades: {shown:?} between {start:?} and {end:?}"
  );
  let (strip, button) = (browser.bounds("browser-strip"), browser.bounds("browser-overflow"));
  let out_of_view = (0..20)
    .map(|index| browser.bounds(&format!("browser-tab-{index}")))
    .filter(|tab| tab.x < strip.x || tab.x + tab.width > button.x)
    .count();
  assert_eq!(browser.text("browser-overflow"), out_of_view.to_string());
}

#[test]
fn browser_tab_strip_shows_the_shown_tab_again_when_the_tabs_go_and_come_back() {
  if !browser_installed() {
    return;
  }
  let mut browser = Browser::open(false);
  browser.feed(with_p15_shown(0..20));
  browser.until_shown("browser-overflow");
  browser.pass();
  assert_tab_15_in_view(&mut browser);

  browser.feed(BrowserEvent::Changed(BrowserState {
    tabs: Vec::new(),
    shown: None,
    mode: BrowserMode::Headless,
  }));
  browser.until("the tabs to go", |browser| !browser.shows("browser-tab-0"));
  browser.feed(with_p15_shown(0..20));
  browser.until_shown("browser-overflow");
  browser.pass();
  assert_tab_15_in_view(&mut browser);
}

#[test]
fn browser_tab_strip_forgets_its_scroll_when_its_tabs_fit_again() {
  if !browser_installed() {
    return;
  }
  let mut browser = Browser::open(false);
  browser.feed(with_p15_shown(0..20));
  browser.until_shown("browser-overflow");
  browser.pass();
  assert_tab_15_in_view(&mut browser);

  browser.feed(with_p15_shown(13..17));
  browser.until("the tabs to fit", |browser| !browser.shows("browser-overflow"));
  browser.pass();
  assert!(
    !browser.shows("browser-tabs-fade-start"),
    "nothing is cut at the start of tabs that fit"
  );
}
