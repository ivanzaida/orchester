//! The tab strip when more tabs are open than fit (`oHEGQ`, `YAOSy`): the tabs that do not fit leave the
//! strip for the tab list, whose button counts them; it, the new-tab button and the drag area stay before
//! the window controls, and the active tab always shows.

use orchester_app::{
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

pub fn shown(window: &mut Window, id: &str) -> bool {
  window.tree.get_element_by_id(id).is_some()
}

/// A window `width` wide with `count` labelled tabs after the first, at All projects, the last active.
pub fn window_with(width: u32, count: usize) -> (Window, Navigation) {
  let mut window = Window::open();
  window.tree.resize(width, 640);
  let navigation = window.navigation();
  let screens = [
    ProjectPage::Overview(OverviewTab::Running),
    ProjectPage::Tasks(TaskList::default()),
    ProjectPage::Pipelines,
    ProjectPage::Secrets,
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
fn a_few_tabs_show_without_the_tab_list() {
  let mut window = Window::open();
  let navigation = window.navigation();
  for _ in 0..3 {
    navigation.open_tab();
    navigation.go(super::project(ProjectPage::Secrets));
  }
  settle(&mut window);
  assert!(!shown(&mut window, "tab-list-button"), "no button while every tab fits");
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
  let (button, new_tab) = (bounds(&mut window, "tab-list-button"), bounds(&mut window, "new-tab"));
  let drag = bounds(&mut window, "title-bar-drag");
  let controls = controls_left(&mut window, 960.0);
  assert!(
    last.x + last.width <= button.x + 0.5,
    "the button follows the last tab: {last:?} {button:?}"
  );
  assert!(
    button.x + button.width <= new_tab.x + 0.5,
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
fn the_button_counts_the_hidden_tabs() {
  let (mut window, navigation) = crowded_window();
  let hidden = tab_ids(&navigation).len() - in_strip(&mut window, &navigation).len();
  assert_eq!(window.text("tab-list-count"), hidden.to_string());
}

#[test]
fn inactive_tabs_stay_120_wide_and_the_active_one_keeps_its_label() {
  let (mut window, navigation) = crowded_window();
  // Kontur's Overview active, and the last inactive tab that shows at Kontur's Instructions, a label
  // longer than an inactive tab's 81.
  let kontur = navigation.tabs_now().tabs()[1].id();
  navigation.select(kontur);
  settle(&mut window);
  let other = in_strip(&mut window, &navigation)
    .into_iter()
    .rev()
    .find(|id| *id != format!("tab-{kontur}"))
    .expect("another tab shows");
  super::press(&mut window, &other, lurq::app::events::MouseButton::Left);
  navigation.go(super::project(ProjectPage::Instructions));
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
  assert!(shown(&mut window, "tab-list-button"));
  window.tree.resize(3000, 640);
  settle(&mut window);
  assert!(!shown(&mut window, "tab-list-button"), "every tab fits a wide window");
  assert_eq!(in_strip(&mut window, &navigation), tab_ids(&navigation));
  window.tree.resize(960, 640);
  settle(&mut window);
  assert!(
    shown(&mut window, "tab-list-button"),
    "and leaves again when it narrows"
  );
  assert!(shown(&mut window, &active_id(&navigation)));
}

#[test]
fn a_short_active_label_leaves_no_room_for_another_tab_unused() {
  let (mut window, navigation) = crowded_window();
  settle(&mut window);
  let shown_tabs = in_strip(&mut window, &navigation);
  let new_tab = bounds(&mut window, "new-tab");
  let end = controls_left(&mut window, 960.0);
  // After New tab: the gap, the 48 of drag area and the title bar's own 4 before the controls.
  let left = end - (new_tab.x + new_tab.width) - 2.0 - 48.0 - 4.0;
  assert!(
    left < 120.0 + 2.0,
    "the {left} left over would hold another 120 wide tab beside {shown_tabs:?}"
  );
}
