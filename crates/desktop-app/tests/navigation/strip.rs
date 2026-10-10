//! The tab strip when more tabs are open than fit: the new-tab button and the drag area stay before the
//! window controls, and the active or new tab is scrolled into view.

use lurq::app::events::{MouseButton, ScrollPhase};
use orchester_app::routes::{OverviewTab, ProjectPage, Route, TaskList};

use super::{PROJECT, press, project, support::Window};

/// The bounds of the element `id`.
fn bounds(window: &mut Window, id: &str) -> lurq::core::ElementRect {
  window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
}

/// A window 960 wide with 12 labelled tabs after the first, at All projects: more than fit.
fn crowded_window() -> (Window, orchester_app::navigation::Navigation) {
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
  window.pass();
  window.pass();
  (window, navigation)
}

/// Where the window controls begin: the laid-out minimize control on Windows. lurq's macOS traffic
/// lights carry no ids; there they take the row's last 8 and three 20.
fn controls_left(window: &mut Window) -> f32 {
  if cfg!(target_os = "macos") {
    960.0 - 68.0
  } else {
    bounds(window, "window-minimize").x
  }
}

#[test]
fn many_tabs_leave_new_tab_and_drag_area_before_the_window_controls() {
  let (mut window, _navigation) = crowded_window();
  let (new_tab, drag) = (bounds(&mut window, "new-tab"), bounds(&mut window, "title-bar-drag"));
  let controls = controls_left(&mut window);
  assert!(drag.width >= 48.0 - 0.5, "the drag area keeps 48: {drag:?}");
  assert!(
    drag.x + drag.width <= controls + 0.5,
    "the drag area ends before the controls at {controls}: {drag:?}"
  );
  assert!(
    new_tab.x + new_tab.width <= drag.x + 0.5,
    "New tab stays visible: {new_tab:?}"
  );
}

/// Whether the tab `id` shows whole in the strip's visible part.
fn in_view(window: &mut Window, id: &str) -> bool {
  let (tab, strip) = (bounds(window, id), bounds(window, "tab-scroll"));
  tab.x >= strip.x - 0.5 && tab.x + tab.width <= strip.x + strip.width + 0.5
}

fn active_id(navigation: &orchester_app::navigation::Navigation) -> String {
  format!("tab-{}", navigation.tabs_now().active().id())
}

/// Turns the mouse wheel over the strip: up (`delta` above 0) scrolls it toward its start.
fn wheel(window: &mut Window, delta: f32) {
  let strip = bounds(window, "tab-scroll");
  let (x, y) = (strip.x + strip.width / 2.0, strip.y + strip.height / 2.0);
  window.tree.mouse_move(x, y);
  window.tree.scroll(x, y, 0.0, delta, ScrollPhase::Scroll);
  window.pass();
  window.pass();
}

fn settle(window: &mut Window) {
  window.pass();
  window.pass();
}

#[test]
fn the_active_tab_is_scrolled_into_view() {
  let (mut window, navigation) = crowded_window();
  let last = active_id(&navigation);
  assert!(in_view(&mut window, &last), "the newest tab shows");
  navigation.cycle(1);
  settle(&mut window);
  assert!(
    in_view(&mut window, "tab-1"),
    "going round to the first tab scrolls back to it"
  );
  navigation.cycle(-1);
  settle(&mut window);
  assert!(in_view(&mut window, &last), "and to the last again");
}

/// The ids of the open tabs, in order.
fn tab_ids(navigation: &orchester_app::navigation::Navigation) -> Vec<String> {
  navigation
    .tabs_now()
    .tabs()
    .iter()
    .map(|tab| format!("tab-{}", tab.id()))
    .collect()
}

#[test]
fn a_new_tab_at_the_right_edge_that_grows_stays_in_view() {
  let (mut window, navigation) = crowded_window();
  wheel(&mut window, 5000.0);
  let strip = bounds(&mut window, "tab-scroll");
  let rightmost = tab_ids(&navigation)
    .into_iter()
    .filter(|id| {
      let tab = bounds(&mut window, id);
      tab.x + tab.width <= strip.x + strip.width + 0.5
    })
    .last()
    .expect("a tab shows at the strip's start");
  press(&mut window, &rightmost, MouseButton::Left);
  settle(&mut window);
  navigation.open_tab();
  settle(&mut window);
  assert!(
    in_view(&mut window, &active_id(&navigation)),
    "the new All projects tab shows at the right edge"
  );
  navigation.go(project(ProjectPage::Instructions));
  settle(&mut window);
  assert!(
    in_view(&mut window, &active_id(&navigation)),
    "and still shows once it is a labelled tab"
  );
}

#[test]
fn moving_in_a_tab_the_wheel_scrolled_away_brings_it_back() {
  let (mut window, navigation) = crowded_window();
  let active = active_id(&navigation);
  wheel(&mut window, 5000.0);
  assert!(!in_view(&mut window, &active), "the wheel scrolled the active tab away");
  navigation.go(project(ProjectPage::Skills));
  settle(&mut window);
  assert!(in_view(&mut window, &active), "going somewhere in it brings it back");
  wheel(&mut window, 5000.0);
  navigation.back();
  settle(&mut window);
  assert!(in_view(&mut window, &active), "so does going back");
}

#[test]
fn a_new_tab_shows_when_the_strip_was_scrolled_to_its_end() {
  let (mut window, navigation) = crowded_window();
  // One more All projects tab at the end leaves the tab cut off on the left at the strip's end with
  // less showing than a new tab is wide, as people see it.
  navigation.open_tab();
  settle(&mut window);
  wheel(&mut window, -5000.0);
  let strip = bounds(&mut window, "tab-scroll");
  let cut = tab_ids(&navigation)
    .into_iter()
    .find(|id| {
      let tab = bounds(&mut window, id);
      tab.x < strip.x && tab.x + tab.width > strip.x
    })
    .expect("a tab is cut off on the left at the strip's end");
  navigation.cycle(-1);
  while active_id(&navigation) != cut {
    navigation.cycle(-1);
  }
  settle(&mut window);
  wheel(&mut window, -5000.0);
  assert!(
    bounds(&mut window, &cut).x < strip.x,
    "the active tab is cut off on the left"
  );
  navigation.open_tab();
  settle(&mut window);
  assert!(in_view(&mut window, &active_id(&navigation)), "the new tab shows whole");
}
