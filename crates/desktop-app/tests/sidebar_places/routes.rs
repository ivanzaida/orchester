//! Where each entry of the old sidebar went (`Where each entry went` `j3WMs9`): its route still reads, and the new
//! sidebar selects the place that holds it, or none where the switcher's menu or the usage card opens it.

use orchester_app::{
  routes::{Route, Screen},
  shell::sidebar::place::Place,
};

use super::{
  overview, selected,
  support::{Window, project_id},
};

const MODULE: &str = "0d3c9d8e-7b6a-4c5d-9e8f-1a2b3c4d5e6f";

/// Each old entry's path under a project, and the place that holds it now.
fn moved() -> Vec<(String, Option<Place>)> {
  [
    ("", Some(Place::Overview)),
    ("/tasks", Some(Place::Tasks)),
    ("/tasks/new", Some(Place::Tasks)),
    ("/tasks/KON-12/summary", Some(Place::Tasks)),
    ("/tasks/KON-12/edit", Some(Place::Tasks)),
    ("/modules", Some(Place::Modules)),
    ("/modules/new", Some(Place::Modules)),
    ("/modules/{module}", Some(Place::Modules)),
    ("/modules/{module}/burndown", Some(Place::Modules)),
    ("/modules/{module}/edit", Some(Place::Modules)),
    ("/orchestrator", Some(Place::Orchestrator)),
    ("/orchestrator/settings", Some(Place::Orchestrator)),
    ("/merge-queue", Some(Place::Review)),
    ("/pull-requests", Some(Place::Review)),
    ("/pull-requests?tab=merged", Some(Place::Review)),
    ("/pull-requests/142", Some(Place::Review)),
    ("/files", Some(Place::Files)),
    ("/files/src/main.rs", Some(Place::Files)),
    ("/pipelines", Some(Place::Pipelines)),
    ("/pipelines/new", Some(Place::Pipelines)),
    ("/agents", Some(Place::Agents)),
    ("/instructions", Some(Place::Agents)),
    ("/skills", Some(Place::Agents)),
    ("/tools", Some(Place::Agents)),
    ("/memory", Some(Place::Agents)),
    ("/usage", None),
    ("/settings", None),
    ("/connections", None),
    ("/secrets", None),
  ]
  .into_iter()
  .map(|(rest, place)| (rest.replace("{module}", MODULE), place))
  .collect()
}

#[test]
fn sidebar_places_every_old_route_reads_and_lands_on_its_new_place() {
  let root = format!("/project/{}", project_id());
  for (rest, place) in moved() {
    let path = format!("{root}{rest}");
    let route = Route::parse(&path).unwrap_or_else(|error| panic!("{path} reads: {error:?}"));
    assert_eq!(Place::of(&route), place, "{path}");
  }
  // Connections and Secrets are tabs of Project settings, Usage opens from the usage card.
  for (rest, screen) in [
    ("/connections", Screen::ProjectSettings),
    ("/secrets", Screen::ProjectSettings),
    ("/settings", Screen::ProjectSettings),
    ("/usage", Screen::Usage),
  ] {
    let route = Route::parse(&format!("{root}{rest}")).expect("the route reads");
    assert_eq!(route.nav_screen(), screen, "{rest}");
  }
  // All projects and Settings are the switcher menu's and the app menu's.
  for path in ["/", "/settings", "/settings?tab=general", "/usage"] {
    let route = Route::parse(path).expect("the route reads");
    assert_eq!(Place::of(&route), None, "{path}");
  }
}

#[test]
fn sidebar_places_each_place_opens_a_route_it_keeps_selected() {
  let project = project_id();
  for place in Place::DAILY.into_iter().chain(Place::CONFIGURE) {
    let route = place.route(project);
    let read = Route::parse(&route.path()).expect("the place's path reads");
    assert_eq!(read, route, "{place:?}");
    assert_eq!(Place::of(&read), Some(place), "{place:?}");
  }
}

#[test]
fn sidebar_places_select_the_entry_of_a_module_and_of_a_hub_tab() {
  let mut window = Window::open();
  let navigation = window.navigation();
  navigation.go(overview(project_id()));
  window.until("the project to open", |window| window.texts("nav-overview").is_some());
  let root = format!("/project/{}", project_id());
  for (rest, entry) in [
    ("/modules", "nav-modules"),
    ("/modules/{module}/burndown", "nav-modules"),
    ("/modules/new", "nav-modules"),
    ("/pull-requests", "nav-review"),
    ("/merge-queue", "nav-review"),
    ("/memory", "nav-agents"),
    ("/skills", "nav-agents"),
    ("/tasks/new", "nav-tasks"),
  ] {
    let path = format!("{root}{}", rest.replace("{module}", MODULE));
    navigation.go(Route::parse(&path).expect("the route reads"));
    window.until(entry, |window| selected(window, entry));
    let others = ["nav-overview", "nav-tasks", "nav-modules", "nav-review", "nav-agents"];
    for other in others.into_iter().filter(|other| *other != entry) {
      assert!(!selected(&mut window, other), "{other} is not selected on {path}");
    }
  }
  navigation.go(Route::parse(&format!("{root}/connections")).expect("the route reads"));
  window.pass();
  for entry in Place::DAILY.into_iter().chain(Place::CONFIGURE) {
    let id = format!("nav-{}", entry.name());
    assert!(
      !selected(&mut window, &id),
      "{id}: Connections is a tab of Project settings"
    );
  }
}
