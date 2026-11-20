//! The tab list (`qWCBQ`, `a1i6Ws`): lurq's list of every open tab under the button of the hidden tabs, every open
//! tab in order, the current one and the hidden ones marked; the pointer and the keyboard switch and close tabs; with
//! more than eight tabs a search field leads it. Ids by lurq's rule: the list `tab-strip-panel`, its search
//! `tab-strip-search`, a row `tab-<id>-item` with `-current`, `-hidden` and `-close` after it.

use lurq::app::events::MouseButton;
use orchester_app::navigation::{Navigation, tabs::TabId};

use super::{
  press,
  strip::{active_id, bounds, crowded_window, in_strip, settle, shown, tab_ids, whole, window_with},
  support::Window,
};

const MORE: &str = "tab-strip-more";
const PANEL: &str = "tab-strip-panel";
const SEARCH: &str = "tab-strip-search";

/// Opens the tab list with the pointer.
fn open_list(window: &mut Window) {
  press(window, MORE, MouseButton::Left);
  settle(window);
  assert!(shown(window, PANEL), "the list opened");
}

fn row(tab: TabId) -> String {
  format!("tab-{tab}-item")
}

fn focused(window: &Window) -> Option<String> {
  window
    .tree
    .focused_element()
    .and_then(|element| element.id().map(str::to_owned))
}

/// Whether the focus is in the search field.
fn in_search(window: &Window) -> bool {
  focused(window).is_some_and(|id| id.starts_with(SEARCH))
}

fn tabs(navigation: &Navigation) -> Vec<TabId> {
  navigation.tabs_now().tabs().iter().map(|tab| tab.id()).collect()
}

/// The first tab that is only in the list.
fn a_hidden_tab(window: &mut Window, navigation: &Navigation) -> TabId {
  tabs(navigation)
    .into_iter()
    .find(|id| !shown(window, &format!("tab-{id}")))
    .expect("a tab is hidden")
}

/// The tab whose row has the focus.
fn ringed(window: &Window, navigation: &Navigation) -> Option<TabId> {
  let at = focused(window)?;
  tabs(navigation).into_iter().find(|tab| at == row(*tab))
}

fn key(window: &mut Window, key: &str) {
  chord(window, key, (false, false));
}

/// Presses `key` with Shift (`shift`) and the platform's command key (`command`: Ctrl, Cmd on macOS).
fn chord(window: &mut Window, key: &str, (command, shift): (bool, bool)) {
  let (ctrl, meta) = (
    command && !cfg!(target_os = "macos"),
    command && cfg!(target_os = "macos"),
  );
  let code = if key.len() == 1 {
    format!("Key{}", key.to_uppercase())
  } else {
    key.to_owned()
  };
  window
    .tree
    .key_down_with_meta(key.to_owned(), code.clone(), shift, ctrl, false, meta);
  window
    .tree
    .key_up_with_meta(key.to_owned(), code, shift, ctrl, false, meta);
  settle(window);
}

/// Types `text` into the focused search field.
fn search(window: &mut Window, text: &str) {
  assert!(in_search(window), "the search field has the focus");
  window.tree.text_input_replace(0..0, text);
  settle(window);
}

#[test]
fn the_list_holds_every_tab_in_order_marking_the_current_and_hidden_ones() {
  let (mut window, navigation) = crowded_window();
  open_list(&mut window);
  let strip = in_strip(&mut window, &navigation);
  let active = navigation.tabs_now().active().id();
  let listed: Vec<TabId> = tabs(&navigation)
    .into_iter()
    .filter(|tab| shown(&mut window, &row(*tab)))
    .collect();
  assert_eq!(listed, tabs(&navigation), "a row for every tab, in order");
  for pair in listed.windows(2) {
    assert!(
      bounds(&mut window, &row(pair[1])).y > bounds(&mut window, &row(pair[0])).y,
      "one under the other"
    );
  }
  for tab in listed {
    let id = row(tab);
    assert_eq!(
      shown(&mut window, &format!("{id}-current")),
      tab == active,
      "{id}: the check"
    );
    let hidden = !strip.contains(&format!("tab-{tab}"));
    assert_eq!(shown(&mut window, &format!("{id}-hidden")), hidden, "{id}: eye-off");
  }
  let kontur = row(tabs(&navigation)[1]);
  window.until("Kontur's name", |window| window.text(&kontur).contains("Kontur"));
  assert_eq!(window.text(&kontur), "Kontur · Overview", "the tab's label");
}

#[test]
fn the_list_opens_under_the_button_with_its_first_rows_in_view() {
  // 1440 wide: the list fits under the button's start.
  let (mut window, navigation) = window_with(1440, 20);
  open_list(&mut window);
  let (button, menu) = (bounds(&mut window, MORE), bounds(&mut window, PANEL));
  // Under the button's start, or as far right as the window lets it.
  let start = button.x.min(1440.0 - 320.0);
  assert!(
    (menu.x - start).abs() < 0.5,
    "its start under the button's: {menu:?} {button:?}"
  );
  assert!(
    (menu.y - (button.y + button.height + 4.0)).abs() < 0.5,
    "4 under the button: {menu:?}"
  );
  assert!((menu.width - 320.0).abs() < 0.5);
  // The search field has the focus, and the rows show from the first.
  let first = bounds(&mut window, &row(tabs(&navigation)[0]));
  assert!(
    first.y >= menu.y && first.y + first.height <= menu.y + menu.height + 0.5,
    "the first row is in view: {first:?} {menu:?}"
  );
}

#[test]
fn a_long_list_leads_with_a_focused_search_that_filters_its_rows() {
  let (mut window, navigation) = crowded_window();
  open_list(&mut window);
  assert!(
    in_search(&window),
    "the search field has the focus: {:?}",
    focused(&window)
  );
  search(&mut window, "envir");
  let matched: Vec<TabId> = tabs(&navigation)
    .into_iter()
    .filter(|tab| shown(&mut window, &row(*tab)))
    .collect();
  assert_eq!(matched.len(), 3, "the three Secrets tabs, Environment, match");
  key(&mut window, "ArrowDown");
  assert_eq!(
    ringed(&window, &navigation),
    Some(matched[0]),
    "Down goes to the first match"
  );
  key(&mut window, "ArrowDown");
  assert_eq!(ringed(&window, &navigation), Some(matched[1]));
  key(&mut window, "Enter");
  assert_eq!(navigation.tabs_now().active().id(), matched[1], "Enter switches to it");
  assert!(!shown(&mut window, PANEL), "and closes the list");
}

#[test]
fn enter_in_the_search_chooses_the_first_match() {
  let (mut window, navigation) = crowded_window();
  open_list(&mut window);
  search(&mut window, "envir");
  let first = tabs(&navigation)
    .into_iter()
    .find(|tab| shown(&mut window, &row(*tab)))
    .expect("a match");
  key(&mut window, "Enter");
  assert_eq!(navigation.tabs_now().active().id(), first);
  assert!(!shown(&mut window, PANEL));
}

#[test]
fn a_search_that_matches_nothing_says_so_and_esc_clears_it_then_closes() {
  let (mut window, navigation) = crowded_window();
  let active = active_id(&navigation);
  open_list(&mut window);
  search(&mut window, "deploy");
  assert!(
    window
      .text(PANEL)
      .contains("No open tab matches \u{201c}deploy\u{201d}"),
    "{}",
    window.text(PANEL)
  );
  key(&mut window, "Escape");
  assert!(shown(&mut window, PANEL), "the first Esc clears the search");
  assert!(!window.text(PANEL).contains("No open tab matches"));
  key(&mut window, "Escape");
  assert!(!shown(&mut window, PANEL), "the second closes the list");
  assert_eq!(active_id(&navigation), active, "without switching");
}

#[test]
fn a_short_list_has_no_search_and_opens_on_the_current_row() {
  let (mut window, navigation) = window_with(600, 5);
  assert!(shown(&mut window, MORE), "six tabs overflow at 600");
  open_list(&mut window);
  assert!(!shown(&mut window, SEARCH), "eight tabs or fewer: no search");
  let current = navigation.tabs_now().active().id();
  assert_eq!(ringed(&window, &navigation), Some(current));
  key(&mut window, "Home");
  assert_eq!(
    ringed(&window, &navigation),
    Some(tabs(&navigation)[0]),
    "Home goes to the first"
  );
  key(&mut window, "End");
  assert_eq!(ringed(&window, &navigation), tabs(&navigation).last().copied());
}

#[test]
fn choosing_a_hidden_tab_in_the_list_brings_it_into_the_strip() {
  let (mut window, navigation) = crowded_window();
  navigation.cycle(1);
  settle(&mut window);
  let hidden = a_hidden_tab(&mut window, &navigation);
  open_list(&mut window);
  press(&mut window, &row(hidden), MouseButton::Left);
  settle(&mut window);
  assert_eq!(navigation.tabs_now().active().id(), hidden, "the chosen tab is active");
  assert!(!shown(&mut window, PANEL), "the list closed");
  assert!(whole(&mut window, &format!("tab-{hidden}")), "it shows in the strip");
}

#[test]
fn a_rows_close_button_and_the_middle_button_close_its_tab_and_keep_the_list_open() {
  let (mut window, navigation) = crowded_window();
  navigation.cycle(1);
  settle(&mut window);
  let (count, active) = (tabs(&navigation).len(), navigation.tabs_now().active().id());
  let hidden = a_hidden_tab(&mut window, &navigation);
  open_list(&mut window);
  let close = format!("{}-close", row(hidden));
  window.hover(&row(hidden));
  press(&mut window, &close, MouseButton::Left);
  settle(&mut window);
  assert_eq!(tabs(&navigation).len(), count - 1, "the tab closed");
  assert_eq!(navigation.tabs_now().active().id(), active, "the active tab stays");
  assert!(shown(&mut window, PANEL), "the list stays open");
  assert!(!shown(&mut window, &row(hidden)), "its row left the list");
  press(&mut window, &row(tabs(&navigation)[2]), MouseButton::Middle);
  settle(&mut window);
  assert_eq!(tabs(&navigation).len(), count - 2, "a middle click closes a row's tab");
}

#[test]
fn the_shortcut_opens_the_list_and_ctrl_w_closes_the_focused_rows_tab_only() {
  let (mut window, navigation) = crowded_window();
  let (count, active) = (tabs(&navigation).len(), navigation.tabs_now().active().id());
  chord(&mut window, "a", (true, true));
  assert!(shown(&mut window, PANEL), "Ctrl+Shift+A opened the list");
  assert!(in_search(&window));
  key(&mut window, "ArrowDown");
  key(&mut window, "ArrowDown");
  let second = tabs(&navigation)[1];
  assert_eq!(ringed(&window, &navigation), Some(second));
  let next = tabs(&navigation)[2];
  chord(&mut window, "w", (true, false));
  assert_eq!(tabs(&navigation).len(), count - 1, "Ctrl+W closed a tab");
  assert!(!tabs(&navigation).contains(&second), "the focused row's");
  assert_eq!(navigation.tabs_now().active().id(), active, "never the active one");
  assert_eq!(
    ringed(&window, &navigation),
    Some(next),
    "the focus moves to the row after it"
  );
  chord(&mut window, "a", (true, true));
  assert!(!shown(&mut window, PANEL), "the shortcut closes it again");
  assert_eq!(tab_ids(&navigation).len(), count - 1);
}

#[test]
fn keys_the_list_takes_do_not_reach_the_page_under_it() {
  let (mut window, navigation) = window_with(1440, 5);
  navigation.select(tabs(&navigation)[0]);
  assert_eq!(navigation.route_now(), orchester_app::routes::Route::AllProjects);
  let row = format!("project-row-{}", super::support::project_id());
  window.until("All projects to list Kontur", |window| {
    window.tree.get_element_by_id(&row).is_some()
  });
  window.hover(&row);
  window.press(&format!("project-remove-{}", super::support::project_id()));
  assert!(
    shown(&mut window, "remove-project-question"),
    "the remove question is open"
  );
  // Narrowed until the six tabs overflow, so the list is a short one without search.
  window.tree.resize(500, 640);
  settle(&mut window);
  assert!(shown(&mut window, "remove-project-question"), "and stays open");
  open_list(&mut window);
  assert!(!shown(&mut window, SEARCH), "a short list: the keys go to its rows");
  key(&mut window, "ArrowDown");
  key(&mut window, "Enter");
  assert_eq!(
    navigation.tabs_now().active().id(),
    tabs(&navigation)[1],
    "Enter switched tabs"
  );
  for _ in 0..20 {
    window.tree.tick_futures();
    settle(&mut window);
  }
  let workspace = window.launch.workspace().expect("the session").clone();
  assert!(
    !workspace.projects().expect("the list").is_empty(),
    "and did not also confirm the removal under the list"
  );
}

#[test]
fn a_list_taller_than_the_window_scrolls() {
  let (mut window, navigation) = crowded_window();
  window.tree.resize(960, 400);
  settle(&mut window);
  open_list(&mut window);
  let menu = bounds(&mut window, PANEL);
  assert!(menu.y + menu.height <= 400.0, "the list stays in the window: {menu:?}");
  let all = tabs(&navigation);
  let inside = |window: &mut Window, id: &str| {
    let (row, menu) = (bounds(window, id), bounds(window, PANEL));
    row.y >= menu.y - 0.5 && row.y + row.height <= menu.y + menu.height + 0.5
  };
  let last = row(*all.last().expect("tabs"));
  assert!(!inside(&mut window, &last), "the last row is out of view at first");
  let at = bounds(&mut window, &row(all[0]));
  window
    .tree
    .scroll(at.x, at.y, 0.0, -5000.0, lurq::app::events::ScrollPhase::Scroll);
  settle(&mut window);
  assert!(inside(&mut window, &last), "the wheel scrolls to it");
}

/// The Tab that comes with Alt+Tab back into the window must not land on the list's button, where Space would open
/// the list.
#[test]
fn the_list_button_is_never_a_tab_stop() {
  let (mut window, _navigation) = crowded_window();
  for _ in 0..40 {
    key(&mut window, "Tab");
    assert_ne!(focused(&window).as_deref(), Some(MORE));
  }
}
