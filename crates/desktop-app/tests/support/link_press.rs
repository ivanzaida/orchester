//! Presses on a link in a chat, and on every control that navigates (`navigation::link`), by a browser's rules
//! (`navigation::press`): a plain click opens its place in the active tab; a middle click and Ctrl+click (Cmd+click
//! on macOS) in a new tab right of the active one, which stays active; Ctrl+Shift+click in a new tab it switches to;
//! Shift+click as a plain click; Enter on the focused link in the active tab. A person's pointer and lurq's synthetic
//! input (an MCP client's `lurq_interact`, with `"button": "middle"` and modifiers) follow the same rules.

use lurq::{
  app::{
    Tree,
    events::MouseButton,
    synthetic_input::{SyntheticInput, SyntheticModifiers, apply},
  },
  core::ElementRect,
};
use orchester_app::{
  navigation::tabs::{TabId, TabSet},
  routes::Route,
};

use super::Window;

/// Who presses a link.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presser {
  /// A person's pointer, as the window's shell delivers it.
  Person,
  /// lurq's synthetic input, as an MCP client's `lurq_interact` delivers it.
  Synthetic,
}

/// The modifiers of a press: the platform's command key (Ctrl, Cmd on macOS) and Shift.
fn modifiers((command, shift): (bool, bool)) -> SyntheticModifiers {
  SyntheticModifiers {
    shift,
    ctrl: command && !cfg!(target_os = "macos"),
    alt: false,
    meta: command && cfg!(target_os = "macos"),
  }
}

/// Presses the middle of the link whose words are `words` with `button`, once it is in place, holding the command
/// key when `command` and Shift when `shift`.
pub fn press_link(window: &mut Window, words: &str, (presser, button): (Presser, MouseButton), keys: (bool, bool)) {
  let bounds = shown(window, words);
  press_at(window, bounds, (presser, button), keys);
}

/// Presses the middle of the element `id` as [`press_link`] presses a link.
pub fn press_control(window: &mut Window, id: &str, (presser, button): (Presser, MouseButton), keys: (bool, bool)) {
  let bounds = shown_id(window, id);
  press_at(window, bounds, (presser, button), keys);
}

fn press_at(window: &mut Window, bounds: ElementRect, (presser, button): (Presser, MouseButton), keys: (bool, bool)) {
  press_in(&mut window.tree, bounds, (presser, button), keys);
  window.pass();
}

/// Presses the middle of `bounds` in `tree` as [`press_link`] presses a link, without laying the tree out again.
pub fn press_in(tree: &mut Tree, bounds: ElementRect, (presser, button): (Presser, MouseButton), keys: (bool, bool)) {
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  let m = modifiers(keys);
  match presser {
    Presser::Person => {
      tree.mouse_move_with_meta(x, y, m.shift, m.ctrl, m.alt, m.meta);
      tree.mouse_down_with_meta(x, y, button, m.shift, m.ctrl, m.alt, m.meta);
      tree.mouse_up_with_meta(x, y, button, m.shift, m.ctrl, m.alt, m.meta);
    }
    Presser::Synthetic => apply(tree, &SyntheticInput::click_button(x, y, button).with_modifiers(m)),
  }
}

/// Presses Enter on the focused element of `tree`, holding the command key (Ctrl, Cmd on macOS) when `command`.
pub fn enter_in(tree: &mut Tree, command: bool) {
  let m = modifiers((command, false));
  tree.key_down_with_meta("Enter".to_owned(), "Enter".to_owned(), m.shift, m.ctrl, m.alt, m.meta);
  tree.key_up_with_meta("Enter".to_owned(), "Enter".to_owned(), m.shift, m.ctrl, m.alt, m.meta);
}

/// Waits until the element `id` is laid out and stays where it is; where it is then.
fn shown_id(window: &mut Window, id: &str) -> ElementRect {
  settled(window, &format!("{id} in place"), |window| {
    window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
  })
}

/// Waits until `bounds` finds an element and it stays where it is, as a page shown again scrolls to where it was and
/// a list lays its rows out again; where it is then.
fn settled(window: &mut Window, what: &str, mut bounds: impl FnMut(&mut Window) -> Option<ElementRect>) -> ElementRect {
  let mut last = None;
  window.until(what, |window| {
    let now = bounds(window);
    let still = now.is_some() && now == last;
    last = now;
    still
  });
  last.expect("the element is in place")
}

/// Waits until the link `words` is shown and stays where it is; where it is then.
fn shown(window: &mut Window, words: &str) -> ElementRect {
  settled(window, &format!("the link {words} in place"), |window| {
    window
      .tree
      .find_element(|element| element.text_content() == Some(words))
      .map(|element| element.bounds())
  })
}

/// Moves the keyboard focus with Tab to the link `words`, then presses Enter.
fn enter_on(window: &mut Window, words: &str) {
  shown(window, words);
  let focused = |window: &mut Window| {
    window
      .tree
      .focused_element()
      .and_then(|element| element.text_content().map(str::to_owned))
  };
  let mut tabs = 0;
  while focused(window).as_deref() != Some(words) {
    assert!(tabs < 500, "Tab reaches the link {words}");
    window
      .tree
      .key_down("Tab".to_owned(), "Tab".to_owned(), false, false, false);
    window
      .tree
      .key_up("Tab".to_owned(), "Tab".to_owned(), false, false, false);
    window.pass();
    tabs += 1;
  }
  window
    .tree
    .key_down("Enter".to_owned(), "Enter".to_owned(), false, false, false);
  window
    .tree
    .key_up("Enter".to_owned(), "Enter".to_owned(), false, false, false);
  window.pass();
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

  // Each new background tab lies right of the active one, before the tabs opened earlier.
  for (presser, button, command) in [
    (Presser::Person, MouseButton::Middle, false),
    (Presser::Synthetic, MouseButton::Middle, false),
    (Presser::Person, MouseButton::Left, true),
    (Presser::Synthetic, MouseButton::Left, true),
  ] {
    press_link(window, words, (presser, button), (command, false));
    expected.insert(at + 1, place.clone());
    let tabs = navigation.tabs_now();
    let press = format!("{presser:?} {button:?} with command {command}");
    assert_eq!(
      places(&tabs),
      expected,
      "{press}: a new tab at the place, right of the active one"
    );
    assert_eq!(tabs.active().id(), chat_tab, "{press}: the active tab stays active");
    assert_eq!(navigation.route_now(), chat, "{press}: and where it was");
  }

  press_link(window, words, (Presser::Person, MouseButton::Right), (false, false));
  assert_eq!(
    places(&navigation.tabs_now()),
    expected,
    "the right button opens nothing"
  );
  assert_eq!(navigation.route_now(), chat);

  // lurq 0.48 takes a Shift press on selectable text to extend the selection an earlier press started; Escape
  // forgets where it started, and Shift+click is then a plain click.
  window
    .tree
    .key_down("Escape".to_owned(), "Escape".to_owned(), false, false, false);
  window
    .tree
    .key_up("Escape".to_owned(), "Escape".to_owned(), false, false, false);
  press_link(window, words, (Presser::Person, MouseButton::Left), (false, true));
  assert_eq!(
    navigation.route_now(),
    *place,
    "Shift+click opens the place in the active tab"
  );
  assert_eq!(navigation.tabs_now().active().id(), chat_tab);
  assert_eq!(navigation.tabs_now().tabs().len(), expected.len(), "and no tab");
  navigation.back();

  for presser in [Presser::Person, Presser::Synthetic] {
    press_link(window, words, (presser, MouseButton::Left), (true, true));
    let tabs = navigation.tabs_now();
    expected.insert(at + 1, place.clone());
    assert_eq!(places(&tabs), expected, "{presser:?} Ctrl+Shift+click opens a new tab");
    assert_eq!(position(&tabs, tabs.active().id()), at + 1, "and switches to it");
    navigation.select(chat_tab);
  }

  enter_on(window, words);
  assert_eq!(
    navigation.route_now(),
    *place,
    "Enter on the focused link opens the place in the active tab"
  );
  assert_eq!(navigation.tabs_now().tabs().len(), expected.len(), "and no tab");
  navigation.back();

  press_link(window, words, (Presser::Synthetic, MouseButton::Left), (false, false));
  assert_eq!(
    navigation.route_now(),
    *place,
    "a plain click opens the place in the active tab"
  );
  assert_eq!(navigation.tabs_now().active().id(), chat_tab);
  assert_eq!(navigation.tabs_now().tabs().len(), expected.len(), "and no tab");
}

/// Every kind of press on the control `id`, which opens `place`, on the page the active tab shows: a middle click and
/// Ctrl+click open `place` in a background tab right of the active one, Ctrl+Shift+click in a new tab there it switches
/// to, by a person's pointer and by synthetic input alike; the right button opens nothing; a plain click opens it in
/// the active tab, as it always did.
pub fn control_opens_by_browser_rules(window: &mut Window, id: &str, place: &Route) {
  shown_id(window, id);
  let navigation = window.navigation();
  let here = navigation.route_now();
  assert_ne!(here, *place, "the control leads away from where the tab is");
  let start = navigation.tabs_now();
  let (home, at) = (start.active().id(), position(&start, start.active().id()));
  let mut expected = places(&start);
  for (presser, button, command) in [
    (Presser::Person, MouseButton::Middle, false),
    (Presser::Synthetic, MouseButton::Middle, false),
    (Presser::Person, MouseButton::Left, true),
    (Presser::Synthetic, MouseButton::Left, true),
  ] {
    press_control(window, id, (presser, button), (command, false));
    expected.insert(at + 1, place.clone());
    let tabs = navigation.tabs_now();
    let press = format!("{presser:?} {button:?} on {id} with command {command}");
    assert_eq!(
      places(&tabs),
      expected,
      "{press}: a new tab at the place, right of the active one"
    );
    assert_eq!(tabs.active().id(), home, "{press}: the active tab stays active");
    assert_eq!(navigation.route_now(), here, "{press}: and where it was");
  }
  press_control(window, id, (Presser::Person, MouseButton::Right), (false, false));
  assert_eq!(
    places(&navigation.tabs_now()),
    expected,
    "the right button opens nothing"
  );
  assert_eq!(navigation.route_now(), here);
  for presser in [Presser::Person, Presser::Synthetic] {
    press_control(window, id, (presser, MouseButton::Left), (true, true));
    let tabs = navigation.tabs_now();
    expected.insert(at + 1, place.clone());
    assert_eq!(
      places(&tabs),
      expected,
      "{presser:?} Ctrl+Shift+click on {id} opens a new tab"
    );
    assert_eq!(position(&tabs, tabs.active().id()), at + 1, "and switches to it");
    assert_eq!(navigation.route_now(), *place);
    navigation.select(home);
  }
  press_control(window, id, (Presser::Person, MouseButton::Left), (false, false));
  assert_eq!(
    navigation.route_now(),
    *place,
    "a plain click on {id} opens the place in the active tab"
  );
  assert_eq!(navigation.tabs_now().active().id(), home);
  assert_eq!(navigation.tabs_now().tabs().len(), expected.len(), "and no tab");
  navigation.back();
  assert_eq!(
    navigation.route_now(),
    here,
    "Back after the plain click on {id} returns to where the tab was"
  );
}
