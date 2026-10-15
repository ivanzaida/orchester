//! The tab strip when more tabs are open than fit: the tabs that do not fit leave the strip for the tab
//! list, whose chevron, the new-tab button and the drag area stay before the window controls; the active
//! tab always shows; the list switches to and closes tabs by pointer and by keyboard.

use lurq::app::events::MouseButton;
use orchester_app::{
  navigation::Navigation,
  routes::{OverviewTab, ProjectPage, Route, TaskList},
};

use super::{PROJECT, press, support::Window};

/// The bounds of the element `id`.
fn bounds(window: &mut Window, id: &str) -> lurq::core::ElementRect {
  window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
}

fn shown(window: &mut Window, id: &str) -> bool {
  window.tree.get_element_by_id(id).is_some()
}

/// A window 960 wide with 12 labelled tabs after the first, at All projects: more than fit.
fn crowded_window() -> (Window, Navigation) {
  let mut window = Window::open();
  window.tree.resize(960, 640);
  let navigation = window.navigation();
  for screen in [
    ProjectPage::Overview(OverviewTab::Running),
    ProjectPage::Tasks(TaskList::default()),
    ProjectPage::Pipelines,
    ProjectPage::Secrets,
  ] {
    for project in [
      PROJECT,
      "0f1e2d3c-4b5a-4968-8776-655443322110",
      "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee",
    ] {
      navigation.open_tab();
      navigation.go(Route::Project(project.parse().expect("a project id"), screen.clone()));
    }
  }
  settle(&mut window);
  (window, navigation)
}

fn settle(window: &mut Window) {
  window.pass();
  window.pass();
}

/// Where the window controls begin: the laid-out minimize control on Windows. lurq's macOS traffic
/// lights carry no ids; there they take the row's last 8 and three 20.
fn controls_left(window: &mut Window, width: f32) -> f32 {
  if cfg!(target_os = "macos") {
    width - 68.0
  } else {
    bounds(window, "window-minimize").x
  }
}

/// The ids of the open tabs, in order.
fn tab_ids(navigation: &Navigation) -> Vec<String> {
  navigation
    .tabs_now()
    .tabs()
    .iter()
    .map(|tab| format!("tab-{}", tab.id()))
    .collect()
}

fn active_id(navigation: &Navigation) -> String {
  format!("tab-{}", navigation.tabs_now().active().id())
}

/// The tabs that show in the strip, in order.
fn in_strip(window: &mut Window, navigation: &Navigation) -> Vec<String> {
  tab_ids(navigation)
    .into_iter()
    .filter(|id| shown(window, id))
    .collect()
}

/// Whether the tab `id` shows whole inside the strip.
fn whole(window: &mut Window, id: &str) -> bool {
  let (tab, strip) = (bounds(window, id), bounds(window, "tab-strip"));
  tab.x >= strip.x - 0.5 && tab.x + tab.width <= strip.x + strip.width + 0.5
}

#[test]
fn a_few_tabs_show_without_the_tab_list() {
  let mut window = Window::open();
  let navigation = window.navigation();
  for _ in 0..3 {
    navigation.open_tab();
    navigation.go(super::project(ProjectPage::Secrets));
  }
  settle(&mut window);
  assert!(!shown(&mut window, "tab-list-button"), "no chevron while every tab fits");
  for id in tab_ids(&navigation) {
    assert!(whole(&mut window, &id), "{id} shows whole");
  }
}

#[test]
fn many_tabs_leave_the_chevron_new_tab_and_drag_area_before_the_window_controls() {
  let (mut window, navigation) = crowded_window();
  let strip = in_strip(&mut window, &navigation);
  assert!(
    strip.len() < tab_ids(&navigation).len(),
    "some tabs left the strip: {strip:?}"
  );
  let last = bounds(&mut window, strip.last().expect("a tab shows"));
  let (chevron, new_tab) = (bounds(&mut window, "tab-list-button"), bounds(&mut window, "new-tab"));
  let drag = bounds(&mut window, "title-bar-drag");
  let controls = controls_left(&mut window, 960.0);
  assert!(
    last.x + last.width <= chevron.x + 0.5,
    "the chevron follows the last tab: {last:?} {chevron:?}"
  );
  assert!(
    chevron.x + chevron.width <= new_tab.x + 0.5,
    "and comes before New tab: {new_tab:?}"
  );
  assert!(new_tab.x + new_tab.width <= drag.x + 0.5, "New tab shows: {drag:?}");
  assert!(drag.width >= 48.0 - 0.5, "the drag area keeps 48: {drag:?}");
  assert!(
    drag.x + drag.width <= controls + 0.5,
    "the drag area ends before the controls at {controls}: {drag:?}"
  );
  for id in strip {
    assert!(whole(&mut window, &id), "{id} shows whole");
  }
}

#[test]
fn the_active_tab_always_shows_in_the_strip() {
  let (mut window, navigation) = crowded_window();
  for _ in 0..tab_ids(&navigation).len() {
    navigation.cycle(1);
    settle(&mut window);
    let active = active_id(&navigation);
    assert!(shown(&mut window, &active), "{active} is in the strip");
    assert!(whole(&mut window, &active), "{active} shows whole");
  }
}

/// Opens the tab list with the pointer.
fn open_list(window: &mut Window) {
  press(window, "tab-list-button", MouseButton::Left);
  settle(window);
  assert!(shown(window, "tab-list-menu"), "the list opened");
}

/// The rows of the open list: each row's tab id and state.
fn rows(window: &mut Window, navigation: &Navigation) -> Vec<(String, String)> {
  navigation
    .tabs_now()
    .tabs()
    .iter()
    .filter_map(|tab| {
      let row = window.tree.get_element_by_id(&format!("tab-list-{}", tab.id()))?;
      let state = if row.has_class("active") { "active" } else { "inactive" };
      Some((format!("tab-{}", tab.id()), state.to_owned()))
    })
    .collect()
}

#[test]
fn the_list_holds_every_tab_in_order_with_the_active_one_marked() {
  let (mut window, navigation) = crowded_window();
  open_list(&mut window);
  let rows = rows(&mut window, &navigation);
  assert_eq!(
    rows.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(),
    tab_ids(&navigation),
    "a row for every tab, in order"
  );
  let active = active_id(&navigation);
  for (id, state) in &rows {
    let expected = if *id == active { "active" } else { "inactive" };
    assert_eq!(state, expected, "{id}");
  }
  let row = bounds(&mut window, &format!("tab-list-{}", navigation.tabs_now().active().id()));
  let menu = bounds(&mut window, "tab-list-menu");
  assert!(
    row.y >= menu.y && row.y + row.height <= menu.y + menu.height + 0.5,
    "the active row is scrolled into the list's view: {row:?} {menu:?}"
  );
}

/// The first tab that is only in the list.
fn a_hidden_tab(window: &mut Window, navigation: &Navigation) -> orchester_app::navigation::tabs::TabId {
  navigation
    .tabs_now()
    .tabs()
    .iter()
    .map(|tab| tab.id())
    .find(|id| !shown(window, &format!("tab-{id}")))
    .expect("a tab is hidden")
}

fn focused(window: &Window) -> Option<String> {
  window
    .tree
    .focused_element()
    .and_then(|element| element.id().map(str::to_owned))
}

#[test]
fn choosing_a_hidden_tab_in_the_list_brings_it_into_the_strip() {
  let (mut window, navigation) = crowded_window();
  navigation.cycle(1);
  settle(&mut window);
  let hidden = a_hidden_tab(&mut window, &navigation);
  open_list(&mut window);
  press(&mut window, &format!("tab-list-{hidden}"), MouseButton::Left);
  settle(&mut window);
  assert_eq!(navigation.tabs_now().active().id(), hidden, "the chosen tab is active");
  assert!(!shown(&mut window, "tab-list-menu"), "the list closed");
  assert!(
    whole(&mut window, &format!("tab-{hidden}")),
    "the chosen tab shows in the strip"
  );
  assert_eq!(focused(&window).as_deref(), Some("tab-list-button"));
}

#[test]
fn closing_a_tab_in_the_list_keeps_the_list_open() {
  let (mut window, navigation) = crowded_window();
  navigation.cycle(1);
  settle(&mut window);
  let (count, active) = (tab_ids(&navigation).len(), navigation.tabs_now().active().id());
  let hidden = a_hidden_tab(&mut window, &navigation);
  open_list(&mut window);
  press(&mut window, &format!("tab-list-{hidden}-close"), MouseButton::Left);
  settle(&mut window);
  assert_eq!(tab_ids(&navigation).len(), count - 1, "the tab closed");
  assert_eq!(navigation.tabs_now().active().id(), active, "the active tab stays");
  assert!(shown(&mut window, "tab-list-menu"), "the list stays open");
  assert!(!shown(&mut window, &format!("tab-list-{hidden}")), "its row left the list");
  press(&mut window, &format!("tab-list-{active}"), MouseButton::Middle);
  settle(&mut window);
  assert_eq!(tab_ids(&navigation).len(), count - 2, "a middle click closes a row's tab");
}

fn key(window: &mut Window, key: &str) {
  window.tree.key_down(key.to_owned(), key.to_owned(), false, false, false);
  window.tree.key_up(key.to_owned(), key.to_owned(), false, false, false);
  settle(window);
}

/// The tab whose row the keyboard ringed.
fn ringed(window: &mut Window, navigation: &Navigation) -> Option<String> {
  navigation.tabs_now().tabs().iter().find_map(|tab| {
    let row = window.tree.get_element_by_id(&format!("tab-list-{}", tab.id()))?;
    row.has_class("ringed").then(|| format!("tab-{}", tab.id()))
  })
}

#[test]
fn the_keyboard_moves_through_the_list_and_chooses() {
  let (mut window, navigation) = crowded_window();
  navigation.cycle(1);
  settle(&mut window);
  let ids = tab_ids(&navigation);
  open_list(&mut window);
  assert_eq!(ringed(&mut window, &navigation), None, "no ring before a key");
  key(&mut window, "ArrowDown");
  assert_eq!(ringed(&mut window, &navigation).as_ref(), Some(&ids[1]), "down from the active tab");
  key(&mut window, "ArrowUp");
  key(&mut window, "ArrowUp");
  assert_eq!(ringed(&mut window, &navigation).as_ref(), Some(&ids[0]), "and up, stopping at the first");
  key(&mut window, "End");
  assert_eq!(ringed(&mut window, &navigation).as_ref(), ids.last(), "End rings the last");
  let last = ids.last().expect("tabs").replacen("tab-", "tab-list-", 1);
  let (row, menu) = (bounds(&mut window, &last), bounds(&mut window, "tab-list-menu"));
  assert!(
    row.y >= menu.y && row.y + row.height <= menu.y + menu.height + 0.5,
    "the list scrolled the ringed row into view: {row:?} {menu:?}"
  );
  key(&mut window, "Enter");
  assert_eq!(active_id(&navigation), *ids.last().expect("tabs"), "Enter switches to it");
  assert!(!shown(&mut window, "tab-list-menu"), "and closes the list");
  assert_eq!(focused(&window).as_deref(), Some("tab-list-button"));
  assert!(whole(&mut window, &active_id(&navigation)), "the chosen tab shows");
}

#[test]
fn escape_closes_the_list_and_gives_the_focus_back_to_the_chevron() {
  let (mut window, navigation) = crowded_window();
  let active = active_id(&navigation);
  open_list(&mut window);
  key(&mut window, "ArrowUp");
  key(&mut window, "Escape");
  assert!(!shown(&mut window, "tab-list-menu"), "Esc closed the list");
  assert_eq!(active_id(&navigation), active, "without switching");
  assert_eq!(focused(&window).as_deref(), Some("tab-list-button"));
  key(&mut window, "Enter");
  assert!(shown(&mut window, "tab-list-menu"), "Enter on the chevron opens it again");
}

#[test]
fn resizing_the_window_recomputes_which_tabs_show() {
  let (mut window, navigation) = crowded_window();
  assert!(shown(&mut window, "tab-list-button"));
  window.tree.resize(3000, 640);
  settle(&mut window);
  assert!(!shown(&mut window, "tab-list-button"), "every tab fits a wide window");
  assert_eq!(in_strip(&mut window, &navigation), tab_ids(&navigation));
  window.tree.resize(960, 640);
  settle(&mut window);
  assert!(shown(&mut window, "tab-list-button"), "and leaves again when it narrows");
  assert!(shown(&mut window, &active_id(&navigation)));
}
