//! Presses on a link in a chat by a browser's rules (`navigation::press`), as a person's pointer makes them: a plain
//! click opens its place in the active tab, Ctrl+click (Cmd+click on macOS) in a new tab right of the active one,
//! which stays active, Ctrl+Shift+click in a new tab it switches to, Shift+click as a plain click; the right button
//! opens nothing, synthetic input follows the same rules, and Enter is a plain click. A middle click never reaches a
//! Markdown link: lurq 0.46 runs a link's press for the left button only.

use lurq::{
  app::events::{MouseButton, with_synthetic_input},
  core::ElementRect,
};
use orchester_app::{
  navigation::{
    press::LinkPress,
    tabs::{TabId, TabSet},
  },
  routes::Route,
};

use super::Window;

/// Presses the middle of the link whose words are `words` with `button`, once it is in place, holding the platform's
/// command key (Ctrl, Cmd on macOS) when `command` and Shift when `shift`.
pub fn press_link(window: &mut Window, words: &str, button: MouseButton, (command, shift): (bool, bool)) {
  let bounds = shown(window, words);
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  let (ctrl, cmd) = (command && !cfg!(target_os = "macos"), command && cfg!(target_os = "macos"));
  let tree = &mut window.tree;
  if cmd {
    tree.key_down_with_meta("Meta".to_owned(), "MetaLeft".to_owned(), shift, false, false, true);
  }
  tree.mouse_move(x, y);
  tree.mouse_down_with_modifiers(x, y, button, shift, ctrl, false);
  tree.mouse_up_with_modifiers(x, y, button, shift, ctrl, false);
  if cmd {
    tree.key_up_with_meta("Meta".to_owned(), "MetaLeft".to_owned(), shift, false, false, false);
  }
  window.pass();
}

/// Waits until the link `words` is shown and stays where it is, as a chat shown again scrolls to where it was and a
/// list lays its rows out again; where it is then.
fn shown(window: &mut Window, words: &str) -> ElementRect {
  let mut last = None;
  window.until(&format!("the link {words} in place"), |window| {
    let bounds = window
      .tree
      .find_element(|element| element.text_content() == Some(words))
      .map(|element| element.bounds());
    let settled = bounds.is_some() && bounds == last;
    last = bounds;
    settled
  });
  last.expect("the link is in place")
}

/// The place of each tab, in order.
fn places(tabs: &TabSet) -> Vec<Route> {
  tabs.tabs().iter().map(|tab| tab.route().clone()).collect()
}

fn position(tabs: &TabSet, id: TabId) -> usize {
  tabs
    .tabs()
    .iter()
    .position(|tab| tab.id() == id)
    .expect("the tab is open")
}

/// Every kind of press on the link `words`, which names `place`, in the chat the active tab shows.
pub fn opens_by_browser_rules(window: &mut Window, words: &str, place: &Route) {
  shown(window, words);
  let navigation = window.navigation();
  let chat = navigation.route_now();
  let start = navigation.tabs_now();
  let (chat_tab, at) = (start.active().id(), position(&start, start.active().id()));
  let mut expected = places(&start);

  // Ctrl+click, twice: each new tab lies right of the active one, before the tabs opened earlier.
  let background = |window: &mut Window, expected: &mut Vec<Route>| {
    press_link(window, words, MouseButton::Left, (true, false));
    expected.insert(at + 1, place.clone());
    let tabs = window.navigation().tabs_now();
    assert_eq!(places(&tabs), *expected, "a new tab at the place, right of the active one");
    assert_eq!(tabs.active().id(), chat_tab, "the active tab stays active");
    assert_eq!(window.navigation().route_now(), chat, "and where it was");
  };
  background(window, &mut expected);
  with_synthetic_input(|| background(window, &mut expected));

  press_link(window, words, MouseButton::Right, (false, false));
  assert_eq!(
    places(&navigation.tabs_now()),
    expected,
    "the right button opens nothing"
  );
  assert_eq!(navigation.route_now(), chat);

  press_link(window, words, MouseButton::Left, (false, true));
  assert_eq!(navigation.route_now(), *place, "Shift+click opens the place in the active tab");
  assert_eq!(navigation.tabs_now().active().id(), chat_tab);
  assert_eq!(navigation.tabs_now().tabs().len(), expected.len(), "and no tab");
  navigation.back();
  shown(window, words);

  press_link(window, words, MouseButton::Left, (true, true));
  let tabs = navigation.tabs_now();
  expected.insert(at + 1, place.clone());
  assert_eq!(places(&tabs), expected, "Ctrl+Shift+click opens a new tab");
  assert_eq!(position(&tabs, tabs.active().id()), at + 1, "and switches to it");
  navigation.select(chat_tab);
  shown(window, words);

  // Enter after a Ctrl+click: lurq presses a focused link for Enter without a pointer press, and that press is a
  // plain one, whatever the pointer's last was.
  background(window, &mut expected);
  assert_ne!(navigation.link_press(), LinkPress::PLAIN);
  window
    .tree
    .key_down("Enter".to_owned(), "Enter".to_owned(), false, false, false);
  window
    .tree
    .key_up("Enter".to_owned(), "Enter".to_owned(), false, false, false);
  window.pass();
  assert_eq!(navigation.link_press(), LinkPress::PLAIN, "Enter presses as a plain click");

  press_link(window, words, MouseButton::Left, (false, false));
  assert_eq!(navigation.route_now(), *place, "a plain click opens the place in the active tab");
  assert_eq!(navigation.tabs_now().active().id(), chat_tab);
  assert_eq!(navigation.tabs_now().tabs().len(), expected.len(), "and no tab");
}
