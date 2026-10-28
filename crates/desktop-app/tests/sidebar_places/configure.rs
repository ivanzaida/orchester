//! The Configure section folds and unfolds Pipelines and Agents, and keeps its fold per project: every tab of the
//! project shows the same, another project its own.

use orchester_core::id::ProjectId;

use super::{
  overview,
  support::{Data, Window, add_project, project_id},
};

/// Whether the window shows Configure's entries, and what its head says.
fn configure(window: &mut Window) -> (bool, String) {
  let unfolded = window.texts("nav-pipelines").is_some() && window.texts("nav-agents").is_some();
  (unfolded, window.text("sidebar-configure"))
}

#[test]
fn sidebar_configure_folds_and_keeps_its_fold_per_project() {
  let data = Data::new();
  let launch = data.launch();
  let workspace = launch.workspace().expect("the session started");
  let other = ProjectId::generate();
  add_project(workspace, (other, "ORC", "Orchester"), &data.repo("repo-orchester"));
  data.add_kontur(workspace);
  let mut window = Window::of(data, launch);
  let navigation = window.navigation();
  navigation.go(overview(project_id()));
  window.until("Kontur's sidebar", |window| window.texts("sidebar-configure").is_some());
  assert_eq!(configure(&mut window), (true, "Configure".to_owned()));
  window.press("sidebar-configure");
  assert_eq!(
    configure(&mut window),
    (false, "Configure · 2".to_owned()),
    "folded, it says how many entries it holds"
  );
  navigation.go(overview(other));
  window.until("the other project's sidebar", |window| {
    window.text("sidebar-configure") == "Configure"
  });
  assert!(configure(&mut window).0, "another project keeps its own fold");
  // A new tab on Kontur shows Kontur's fold.
  navigation.open_tab();
  navigation.go(overview(project_id()));
  window.until("Kontur's fold", |window| {
    window.text("sidebar-configure") == "Configure · 2"
  });
  assert!(!configure(&mut window).0);
  window.press("sidebar-configure");
  assert_eq!(configure(&mut window), (true, "Configure".to_owned()));
  window.press("nav-pipelines");
  assert_eq!(
    navigation.route_now(),
    orchester_app::shell::sidebar::place::Place::Pipelines.route(project_id())
  );
}
