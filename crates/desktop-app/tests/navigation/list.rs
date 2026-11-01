//! The tab list (`qWCBQ`, `a1i6Ws`): every open tab in order with its page and project, the current one
//! marked, the hidden ones marked; the pointer and the keyboard switch and close tabs; with more than
//! eight tabs a search field leads it.

use lurq::app::events::MouseButton;
use orchester_app::navigation::{Navigation, tabs::TabId};

use super::{
  press,
  strip::{active_id, bounds, crowded_window, in_strip, settle, shown, tab_ids, whole, window_with},
  support::Window,
};

/// Opens the tab list with the pointer.
fn open_list(window: &mut Window) {
  press(window, "tab-list-button", MouseButton::Left);
  settle(window);
  assert!(shown(window, "tab-list-menu"), "the list opened");
}

fn row(tab: TabId) -> String {
  format!("tab-list-{tab}")
}

fn has_class(window: &mut Window, id: &str, class: &str) -> bool {
  window
    .tree
    .get_element_by_id(id)
    .is_some_and(|element| element.has_class(class))
}

fn focused(window: &Window) -> Option<String> {
  window
    .tree
    .focused_element()
    .and_then(|element| element.id().map(str::to_owned))
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

/// The tab whose row the keyboard ringed.
fn ringed(window: &mut Window, navigation: &Navigation) -> Option<TabId> {
  tabs(navigation)
    .into_iter()
    .find(|tab| has_class(window, &row(*tab), "ringed"))
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
  for tab in listed {
    let id = row(tab);
    assert_eq!(has_class(&mut window, &id, "current"), tab == active, "{id}");
    assert_eq!(
      shown(&mut window, &format!("{id}-current")),
      tab == active,
      "{id}: the check"
    );
    let hidden = !strip.contains(&format!("tab-{tab}"));
    assert_eq!(shown(&mut window, &format!("{id}-hidden")), hidden, "{id}: eye-off");
  }
  let first = row(tabs(&navigation)[0]);
  assert_eq!(window.text(&first), "All projects", "All projects names no project");
  let kontur = row(tabs(&navigation)[1]);
  window.until("Kontur's name", |window| window.text(&kontur).contains("Kontur"));
  assert_eq!(window.text(&kontur), "Overview Kontur", "the page, then the project");
}

#[test]
fn the_list_opens_under_the_button_with_its_rows_in_view() {
  // 1440 wide: the list fits under the button's start.
  let (mut window, navigation) = window_with(1440, 20);
  open_list(&mut window);
  let (button, menu) = (
    bounds(&mut window, "tab-list-button"),
    bounds(&mut window, "tab-list-menu"),
  );
  assert!(
    (menu.x - button.x).abs() < 0.5,
    "its start under the button's: {menu:?} {button:?}"
  );
  assert!((menu.y - 40.0).abs() < 0.5, "4 under the title bar: {menu:?}");
  assert!((menu.width - 320.0).abs() < 0.5);
  let current = bounds(&mut window, &row(navigation.tabs_now().active().id()));
  assert!(
    current.y >= menu.y && current.y + current.height <= menu.y + menu.height + 0.5,
    "the current row is in view: {current:?} {menu:?}"
  );
}

#[test]
fn a_long_list_leads_with_a_focused_search_that_filters_its_rows() {
  let (mut window, navigation) = crowded_window();
  open_list(&mut window);
  assert_eq!(focused(&window).as_deref(), Some("tab-list-search"));
  assert_eq!(
    ringed(&mut window, &navigation),
    None,
    "no ring before a key or a search"
  );
  window.set("tab-list-search", "secr");
  settle(&mut window);
  let matched: Vec<TabId> = tabs(&navigation)
    .into_iter()
    .filter(|tab| shown(&mut window, &row(*tab)))
    .collect();
  assert_eq!(matched.len(), 3, "the three Secrets tabs match");
  assert_eq!(
    ringed(&mut window, &navigation),
    Some(matched[0]),
    "the first match is ringed"
  );
  key(&mut window, "ArrowDown");
  assert_eq!(ringed(&mut window, &navigation), Some(matched[1]));
  key(&mut window, "Enter");
  assert_eq!(navigation.tabs_now().active().id(), matched[1], "Enter switches to it");
  assert!(!shown(&mut window, "tab-list-menu"), "and closes the list");
  assert_eq!(focused(&window).as_deref(), Some("tab-list-button"));
}

#[test]
fn a_search_that_matches_nothing_says_so_and_esc_clears_it_then_closes() {
  let (mut window, navigation) = crowded_window();
  let active = active_id(&navigation);
  open_list(&mut window);
  window.set("tab-list-search", "deploy");
  settle(&mut window);
  assert_eq!(
    window.text("tab-list-empty"),
    "No open tab matches \u{201c}deploy\u{201d}"
  );
  key(&mut window, "Escape");
  assert!(shown(&mut window, "tab-list-menu"), "the first Esc clears the search");
  assert!(!shown(&mut window, "tab-list-empty"));
  key(&mut window, "Escape");
  assert!(!shown(&mut window, "tab-list-menu"), "the second closes the list");
  assert_eq!(active_id(&navigation), active, "without switching");
  assert_eq!(focused(&window).as_deref(), Some("tab-list-button"));
}

#[test]
fn a_short_list_has_no_search_and_opens_with_the_current_row_ringed() {
  let (mut window, navigation) = window_with(600, 5);
  assert!(shown(&mut window, "tab-list-button"), "six tabs overflow at 600");
  open_list(&mut window);
  assert!(!shown(&mut window, "tab-list-search"), "eight tabs or fewer: no search");
  let current = navigation.tabs_now().active().id();
  assert_eq!(ringed(&mut window, &navigation), Some(current));
  key(&mut window, "Home");
  assert_eq!(
    ringed(&mut window, &navigation),
    Some(tabs(&navigation)[0]),
    "Home rings the first"
  );
  key(&mut window, "ArrowUp");
  assert_eq!(
    ringed(&mut window, &navigation),
    Some(tabs(&navigation)[0]),
    "and stops there"
  );
  key(&mut window, "End");
  assert_eq!(ringed(&mut window, &navigation), tabs(&navigation).last().copied());
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
  assert!(!shown(&mut window, "tab-list-menu"), "the list closed");
  assert!(whole(&mut window, &format!("tab-{hidden}")), "it shows in the strip");
  assert_eq!(focused(&window).as_deref(), Some("tab-list-button"));
}

/// Moves the pointer over the middle of `id`.
fn hover(window: &mut Window, id: &str) {
  let at = bounds(window, id);
  window.tree.mouse_move(at.x + at.width / 2.0, at.y + at.height / 2.0);
  settle(window);
}

#[test]
fn a_row_shows_its_close_button_under_the_pointer_and_closing_keeps_the_list_open() {
  let (mut window, navigation) = crowded_window();
  navigation.cycle(1);
  settle(&mut window);
  let (count, active) = (tabs(&navigation).len(), navigation.tabs_now().active().id());
  let hidden = a_hidden_tab(&mut window, &navigation);
  open_list(&mut window);
  let close = format!("{}-close", row(hidden));
  assert!(!shown(&mut window, &close), "no close button at rest");
  hover(&mut window, &row(hidden));
  assert!(shown(&mut window, &close), "the hovered row shows its close button");
  press(&mut window, &close, MouseButton::Left);
  settle(&mut window);
  assert_eq!(tabs(&navigation).len(), count - 1, "the tab closed");
  assert_eq!(navigation.tabs_now().active().id(), active, "the active tab stays");
  assert!(shown(&mut window, "tab-list-menu"), "the list stays open");
  assert!(!shown(&mut window, &row(hidden)), "its row left the list");
  press(&mut window, &row(tabs(&navigation)[2]), MouseButton::Middle);
  settle(&mut window);
  assert_eq!(tabs(&navigation).len(), count - 2, "a middle click closes a row's tab");
}

#[test]
fn the_shortcut_opens_the_list_and_ctrl_w_closes_ringed_tabs_only() {
  let (mut window, navigation) = crowded_window();
  let (count, active) = (tabs(&navigation).len(), navigation.tabs_now().active().id());
  chord(&mut window, "a", (true, true));
  assert!(shown(&mut window, "tab-list-menu"), "Ctrl+Shift+A opened the list");
  assert_eq!(focused(&window).as_deref(), Some("tab-list-search"));
  chord(&mut window, "w", (true, false));
  assert_eq!(tabs(&navigation).len(), count, "with no ring, Ctrl+W closes nothing");
  key(&mut window, "ArrowDown");
  assert_eq!(
    ringed(&mut window, &navigation),
    Some(active),
    "the first arrow rings the current row"
  );
  key(&mut window, "ArrowUp");
  key(&mut window, "ArrowUp");
  let first = ringed(&mut window, &navigation).expect("a row is ringed");
  let next = tabs(&navigation)[count - 2];
  chord(&mut window, "w", (true, false));
  assert_eq!(tabs(&navigation).len(), count - 1, "Ctrl+W closed a tab");
  assert!(!tabs(&navigation).contains(&first), "the ringed one");
  assert_eq!(
    ringed(&mut window, &navigation),
    Some(next),
    "the ring moves to the row after it"
  );
  chord(&mut window, "w", (true, false));
  assert_eq!(tabs(&navigation).len(), count - 2, "a second Ctrl+W closes that one");
  assert!(!tabs(&navigation).contains(&next));
  assert_eq!(navigation.tabs_now().active().id(), active, "never the active one");
  chord(&mut window, "a", (true, true));
  assert!(!shown(&mut window, "tab-list-menu"), "the shortcut closes it again");
  assert_eq!(focused(&window).as_deref(), Some("tab-list-button"));
  assert_eq!(tab_ids(&navigation).len(), count - 2);
}

/// A window 600 wide, `height` high, with five labelled tabs after All projects; the last active.
fn narrow_window(height: u32) -> (Window, Navigation) {
  let (mut window, navigation) = window_with(600, 5);
  window.tree.resize(600, height);
  settle(&mut window);
  (window, navigation)
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
  assert!(
    !shown(&mut window, "tab-list-search"),
    "a short list: the keys go to its rows"
  );
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
fn the_list_stays_8_inside_the_window_near_its_right_edge() {
  let (mut window, _navigation) = narrow_window(640);
  open_list(&mut window);
  let (button, menu) = (
    bounds(&mut window, "tab-list-button"),
    bounds(&mut window, "tab-list-menu"),
  );
  assert!(button.x + 320.0 > 592.0, "the button is close to the edge: {button:?}");
  assert!(
    (menu.x + menu.width - 592.0).abs() < 0.5,
    "the list ends 8 inside the window: {menu:?}"
  );
}

#[test]
fn a_list_taller_than_the_window_less_48_scrolls_and_keeps_its_place() {
  let (mut window, navigation) = crowded_window();
  window.tree.resize(960, 400);
  settle(&mut window);
  open_list(&mut window);
  let menu = bounds(&mut window, "tab-list-menu");
  assert!(
    menu.height <= 400.0 - 48.0 + 0.5,
    "the list is at most the window less 48: {menu:?}"
  );
  let all = tabs(&navigation);
  let (first, current) = (row(all[0]), row(*all.last().expect("tabs")));
  let inside = |window: &mut Window, id: &str| {
    let (row, menu) = (bounds(window, id), bounds(window, "tab-list-menu"));
    row.y >= menu.y - 0.5 && row.y + row.height <= menu.y + menu.height + 0.5
  };
  assert!(
    inside(&mut window, &current),
    "the current row, last, is scrolled into view"
  );
  assert!(!inside(&mut window, &first), "so the first one is out of it");
  let at = bounds(&mut window, &current);
  window
    .tree
    .scroll(at.x, at.y, 0.0, 5000.0, lurq::app::events::ScrollPhase::Scroll);
  settle(&mut window);
  assert!(inside(&mut window, &first), "the wheel scrolled to the top");
  let button = bounds(&mut window, "tab-list-button");
  window
    .tree
    .mouse_move(button.x + button.width / 2.0, button.y + button.height / 2.0);
  settle(&mut window);
  assert!(
    inside(&mut window, &first),
    "and the list stays there as it renders again"
  );
}

#[test]
fn new_tab_does_not_take_the_focus_when_the_list_goes_away() {
  let (mut window, navigation) = narrow_window(640);
  open_list(&mut window);
  while shown(&mut window, "tab-list-button") {
    let hidden = a_hidden_tab(&mut window, &navigation);
    press(&mut window, &row(hidden), MouseButton::Middle);
    settle(&mut window);
  }
  assert_ne!(focused(&window).as_deref(), Some("new-tab"));
}

/// The Tab that comes with Alt+Tab back into the window must not land on *New tab*, where Space would open a tab.
#[test]
fn new_tab_is_never_a_tab_stop() {
  let (mut window, _navigation) = crowded_window();
  for _ in 0..40 {
    key(&mut window, "Tab");
    assert_ne!(focused(&window).as_deref(), Some("new-tab"));
  }
  press(&mut window, "new-tab", MouseButton::Left);
  assert_ne!(focused(&window).as_deref(), Some("new-tab"), "nor after a press");
}
