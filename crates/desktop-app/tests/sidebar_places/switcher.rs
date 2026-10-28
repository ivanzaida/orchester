//! The switcher's menu (`Switcher menu` `NEHAB`): after RECENT, Project settings with its keys, All projects,
//! Settings, Open folder… and Add project; each opens its place. Ctrl+, (Cmd+, on macOS) opens Project settings from
//! anywhere in the project.

use orchester_app::routes::{ProjectPage, ProjectSettingsTab, Route};

use super::{
  command, overview,
  support::{Window, project_id},
};

/// The top of the element `id`.
fn top(window: &mut Window, id: &str) -> f32 {
  window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
    .y
}

#[test]
fn sidebar_switcher_menu_holds_project_settings_all_projects_and_settings() {
  let mut window = Window::open();
  let navigation = window.navigation();
  navigation.go(overview(project_id()));
  window.until("the project to open", |window| window.texts("nav-overview").is_some());
  window.press("project-switcher");
  window.until("the menu", |window| window.texts("project-switcher-menu").is_some());
  let order = [
    "switcher-project-settings",
    "switcher-all-projects",
    "switcher-settings",
    "switcher-open-folder",
    "switcher-add-project",
  ];
  let tops: Vec<f32> = order.iter().map(|id| top(&mut window, id)).collect();
  assert!(tops.windows(2).all(|pair| pair[0] < pair[1]), "{order:?}: {tops:?}");
  let keys = if cfg!(target_os = "macos") { "⌘ ," } else { "Ctrl ," };
  let row = window
    .texts("switcher-project-settings")
    .expect("Project settings shows");
  assert!(
    row.iter().any(|text| text == "Project settings") && row.last().is_some_and(|text| text == keys),
    "{row:?}"
  );
  let settings = Route::Project(project_id(), ProjectPage::Settings(ProjectSettingsTab::General));
  window.press("switcher-project-settings");
  assert_eq!(navigation.route_now(), settings);
  assert!(
    window.texts("project-switcher-menu").is_none(),
    "a choice closes the menu"
  );
  for (item, route) in [
    ("switcher-settings", Route::Settings),
    ("switcher-all-projects", Route::AllProjects),
  ] {
    navigation.go(overview(project_id()));
    window.until("the sidebar", |window| window.texts("project-switcher").is_some());
    window.press("project-switcher");
    window.until(item, |window| window.texts(item).is_some());
    window.press(item);
    assert_eq!(navigation.route_now(), route, "{item}");
  }
  navigation.go(overview(project_id()));
  window.until("the sidebar", |window| window.texts("nav-overview").is_some());
  command(&mut window, (",", "Comma"));
  assert_eq!(navigation.route_now(), settings, "Ctrl+, opens Project settings");
}
