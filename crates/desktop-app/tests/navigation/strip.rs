//! The tab strip when more tabs are open than fit (`oHEGQ`, `YAOSy`): the tabs that do not fit leave the
//! strip for the menu of the button after the tabs, which counts them (lurq's window tabs in their list mode, ids
//! by its part rule: `tab-strip-more`, its count `tab-strip-more_label`, *New tab* `tab-strip-add`); it, *New tab*
//! and the drag area stay before the window controls, and the active tab always shows.

use lurq::app::events::MouseButton;
use orchester_app::{
  chrome::{BAR_GAP, END_RESERVE},
  navigation::Navigation,
  routes::{OverviewTab, ProjectPage, Route, TaskList},
};

use super::{PROJECT, support::Window};

/// The bounds of the element `id`.
pub fn bounds(window: &mut Window, id: &str) -> lurq::core::ElementRect {
  window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
}

/// Whether the element `id` is laid out: a tab that left the strip, and the button while none did, take no room.
pub fn shown(window: &mut Window, id: &str) -> bool {
  window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .is_some_and(|bounds| bounds.width > 0.5)
}

/// The button of the tabs that left the strip, *New tab*, and the button's count.
const MORE: &str = "tab-strip-more";
const NEW_TAB: &str = "tab-strip-add";
const COUNT: &str = "tab-strip-more_label";

/// A window `width` wide with `count` labelled tabs after the first, at All projects, the last active.
pub fn window_with(width: u32, count: usize) -> (Window, Navigation) {
  let mut window = Window::open();
  window.tree.resize(width, 640);
  let navigation = window.navigation();
  let screens = [
    ProjectPage::Overview(OverviewTab::Running),
    ProjectPage::Tasks(TaskList::default()),
    ProjectPage::Pipelines,
    ProjectPage::Secrets(Default::default()),
  ];
  let projects = [
    PROJECT,
    "0f1e2d3c-4b5a-4968-8776-655443322110",
    "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee",
  ];
  for index in 0..count {
    let project = projects[index % projects.len()].parse().expect("a project id");
    navigation.open_tab();
    navigation.go(Route::Project(
      project,
      screens[index / projects.len() % screens.len()].clone(),
    ));
  }
  settle(&mut window);
  (window, navigation)
}

/// A window 960 wide with 12 labelled tabs after the first, at All projects: more than fit.
pub fn crowded_window() -> (Window, Navigation) {
  window_with(960, 12)
}

pub fn settle(window: &mut Window) {
  window.pass();
  window.pass();
}

/// Where the title bar's room that drags the window ends, at the latest, in a window `width` wide whose *N need you*
/// pill does not show: before the gap to the pill's place and the bar's end (the Windows controls; on macOS, whose
/// lights are at the bar's start, its last gaps).
fn drag_end(width: f32) -> f32 {
  width - END_RESERVE - BAR_GAP
}

/// The ids of the open tabs, in order.
pub fn tab_ids(navigation: &Navigation) -> Vec<String> {
  navigation
    .tabs_now()
    .tabs()
    .iter()
    .map(|tab| format!("tab-{}", tab.id()))
    .collect()
}

pub fn active_id(navigation: &Navigation) -> String {
  format!("tab-{}", navigation.tabs_now().active().id())
}

/// The tabs that show in the strip, in order.
pub fn in_strip(window: &mut Window, navigation: &Navigation) -> Vec<String> {
  tab_ids(navigation).into_iter().filter(|id| shown(window, id)).collect()
}

/// Whether the tab `id` shows whole inside the strip.
pub fn whole(window: &mut Window, id: &str) -> bool {
  let (tab, strip) = (bounds(window, id), bounds(window, "tab-strip"));
  tab.x >= strip.x - 0.5 && tab.x + tab.width <= strip.x + strip.width + 0.5
}

#[test]
fn a_few_tabs_show_without_the_button_of_hidden_tabs() {
  let mut window = Window::open();
  let navigation = window.navigation();
  for _ in 0..3 {
    navigation.open_tab();
    navigation.go(super::project(ProjectPage::Secrets(Default::default())));
  }
  settle(&mut window);
  assert!(!shown(&mut window, MORE), "no button while every tab fits");
  for id in tab_ids(&navigation) {
    assert!(whole(&mut window, &id), "{id} shows whole");
  }
}

#[test]
fn many_tabs_leave_the_button_new_tab_and_drag_area_before_the_window_controls() {
  let (mut window, navigation) = crowded_window();
  let strip = in_strip(&mut window, &navigation);
  assert!(
    strip.len() < tab_ids(&navigation).len(),
    "some tabs left the strip: {strip:?}"
  );
  let last = bounds(&mut window, strip.last().expect("a tab shows"));
  let (button, new_tab) = (bounds(&mut window, MORE), bounds(&mut window, NEW_TAB));
  let end = drag_end(960.0);
  assert!(
    last.x + last.width <= button.x + 0.5,
    "the button follows the last tab: {last:?} {button:?}"
  );
  assert!(
    button.x + button.width <= new_tab.x + 0.5,
    "and comes before New tab: {new_tab:?}"
  );
  let drag = end - (new_tab.x + new_tab.width);
  assert!(
    drag >= 48.0 - 0.5,
    "the drag area after New tab keeps 48 before {end}: {drag}"
  );
  for id in strip {
    assert!(whole(&mut window, &id), "{id} shows whole");
  }
}

#[test]
fn the_button_counts_the_hidden_tabs() {
  let (mut window, navigation) = crowded_window();
  let hidden = tab_ids(&navigation).len() - in_strip(&mut window, &navigation).len();
  assert_eq!(window.text(COUNT), hidden.to_string());
  // The words of its tooltip, which lurq follows with the keys of the list.
  let says = window
    .tree
    .get_element_by_id_mut("tab-strip-more")
    .and_then(|element| element.description("a11y_description"));
  assert_eq!(says, Some(format!("{hidden} more tabs")));
}

#[test]
fn inactive_tabs_are_at_most_120_wide_and_the_active_one_keeps_its_label() {
  let (mut window, navigation) = crowded_window();
  // Kontur's Overview active, and Kontur's Tasks, which shows beside it, at Kontur's Project settings, a label
  // longer than an inactive tab's 81.
  let kontur = navigation.tabs_now().tabs()[1].id();
  let other = navigation.tabs_now().tabs()[4].id();
  navigation.select(other);
  settle(&mut window);
  navigation.go(super::project(ProjectPage::Settings(Default::default())));
  navigation.select(kontur);
  let active = active_id(&navigation);
  window.until("Kontur's name", |window| window.text(&active).contains("Kontur"));
  let inactive: Vec<f32> = in_strip(&mut window, &navigation)
    .into_iter()
    .filter(|id| *id != active)
    .map(|id| bounds(&mut window, &id).width)
    .collect();
  assert!(
    inactive.iter().all(|width| *width <= 120.5),
    "no inactive tab is wider than 120: {inactive:?}"
  );
  assert!(
    inactive.iter().any(|width| (width - 120.0).abs() < 0.5),
    "a cut label leaves its tab 120 wide: {inactive:?}"
  );
  assert!(
    inactive.iter().any(|width| *width < 119.5),
    "a short label leaves its tab narrower: {inactive:?}"
  );
  assert!(
    bounds(&mut window, &active).width > 120.0,
    "the active tab does not shrink"
  );
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

#[test]
fn resizing_the_window_recomputes_which_tabs_show() {
  let (mut window, navigation) = crowded_window();
  assert!(shown(&mut window, MORE));
  window.tree.resize(3000, 640);
  settle(&mut window);
  assert!(!shown(&mut window, MORE), "every tab fits a wide window");
  assert_eq!(in_strip(&mut window, &navigation), tab_ids(&navigation));
  window.tree.resize(960, 640);
  settle(&mut window);
  assert!(shown(&mut window, MORE), "and leaves again when it narrows");
  assert!(shown(&mut window, &active_id(&navigation)));
}

#[test]
fn a_short_active_label_leaves_no_room_for_another_tab_unused() {
  let (mut window, navigation) = crowded_window();
  settle(&mut window);
  let shown_tabs = in_strip(&mut window, &navigation);
  let new_tab = bounds(&mut window, NEW_TAB);
  let end = drag_end(960.0);
  // After New tab: the 48 of drag area.
  let left = end - (new_tab.x + new_tab.width) - 48.0;
  assert!(
    left < 120.0 + 2.0,
    "the {left} left over would hold another 120 wide tab beside {shown_tabs:?}"
  );
}

#[test]
fn choosing_a_hidden_tab_in_the_menu_brings_it_into_the_strip() {
  let (mut window, navigation) = crowded_window();
  let hidden = navigation
    .tabs_now()
    .tabs()
    .iter()
    .map(|tab| tab.id())
    .find(|id| !shown(&mut window, &format!("tab-{id}")))
    .expect("a tab is hidden");
  super::press(&mut window, MORE, MouseButton::Left);
  settle(&mut window);
  super::press(&mut window, &format!("tab-{hidden}-item"), MouseButton::Left);
  settle(&mut window);
  assert_eq!(navigation.tabs_now().active().id(), hidden, "the chosen tab is active");
  assert!(whole(&mut window, &format!("tab-{hidden}")), "it shows in the strip");
}

/// The Tab that comes with Alt+Tab back into the window must not land on a tab, *New tab* or the button, where
/// Space would open or switch a tab.
#[test]
fn the_strip_is_never_a_tab_stop() {
  let (mut window, navigation) = crowded_window();
  let strip: Vec<String> = tab_ids(&navigation)
    .into_iter()
    .chain([MORE.to_owned(), NEW_TAB.to_owned()])
    .collect();
  let focused = |window: &Window| {
    window
      .tree
      .focused_element()
      .and_then(|element| element.id().map(str::to_owned))
  };
  for _ in 0..40 {
    window
      .tree
      .key_down("Tab".to_owned(), "Tab".to_owned(), false, false, false);
    window
      .tree
      .key_up("Tab".to_owned(), "Tab".to_owned(), false, false, false);
    settle(&mut window);
    let at = focused(&window);
    assert!(
      !at.as_ref().is_some_and(|id| strip.contains(id)),
      "Tab stopped on {at:?}"
    );
  }
  super::press(&mut window, NEW_TAB, MouseButton::Left);
  assert_ne!(focused(&window).as_deref(), Some(NEW_TAB), "nor after a press");
}

#[test]
fn tabs_stand_2_apart_and_new_tab_2_after_them() {
  let (mut window, navigation) = crowded_window();
  let strip = in_strip(&mut window, &navigation);
  for pair in strip.windows(2) {
    let (left, right) = (bounds(&mut window, &pair[0]), bounds(&mut window, &pair[1]));
    assert!(
      (right.x - (left.x + left.width) - 2.0).abs() < 0.5,
      "{} and {}: {left:?} {right:?}",
      pair[0],
      pair[1]
    );
  }
  let last = bounds(&mut window, strip.last().expect("a tab shows"));
  let button = bounds(&mut window, MORE);
  assert!(
    (button.x - (last.x + last.width) - 2.0).abs() < 0.5,
    "the button 2 after the last tab: {last:?} {button:?}"
  );
  window.tree.resize(3000, 640);
  settle(&mut window);
  let last = bounds(&mut window, tab_ids(&navigation).last().expect("tabs"));
  let new_tab = bounds(&mut window, NEW_TAB);
  assert!(
    (new_tab.x - (last.x + last.width) - 2.0).abs() < 0.5,
    "New tab 2 after the last tab: {last:?} {new_tab:?}"
  );
}
