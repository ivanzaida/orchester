//! Review's routes: its list at `/review` and a task's review at `/review/<key>`.

use orchester_app::{
  mcp::tools::route_json,
  routes::{ProjectPage, Route, RouteError, Screen, routes},
};
use serde_json::json;

use super::{PROJECT, at, key, matched};

#[test]
fn review_routes_have_their_documented_paths_and_read_back() {
  let root = format!("/project/{PROJECT}");
  for (route, path) in [
    (at(ProjectPage::Review), format!("{root}/review")),
    (
      at(ProjectPage::TaskReview(key("KON-12"))),
      format!("{root}/review/KON-12"),
    ),
  ] {
    assert_eq!(route.path(), path);
    assert_eq!(Route::parse(&path), Ok(route.clone()), "{path}");
    assert_eq!(route.screen(), Screen::Review);
    assert_eq!(route.nav_screen(), Screen::Review);
  }
  assert_eq!(ProjectPage::of(Screen::Review), Some(ProjectPage::Review));
  assert_eq!(Screen::from_name("review"), Some(Screen::Review));
}

#[test]
fn review_routes_refuse_what_names_no_review() {
  let root = format!("/project/{PROJECT}/review");
  assert!(matches!(
    Route::parse(&format!("{root}/kon-12")),
    Err(RouteError::TaskKey { .. })
  ));
  for path in [format!("{root}/KON-12/more"), format!("{root}/")] {
    assert_eq!(Route::parse(&path), Err(RouteError::NoScreen), "{path}");
  }
  assert!(matches!(
    Route::parse(&format!("{root}?tab=all")),
    Err(RouteError::UnexpectedParam { .. })
  ));
}

#[test]
fn the_router_table_resolves_review_and_a_tasks_review_apart() {
  let table = routes();
  let list = at(ProjectPage::Review);
  let task = at(ProjectPage::TaskReview(key("KON-12")));
  assert_eq!(matched(&table.resolve(&list.path())), list.pattern());
  assert_eq!(matched(&table.resolve(&task.path())), task.pattern());
  assert_ne!(list.pattern(), task.pattern());
}

#[test]
fn the_tools_name_review_routes_as_navigate_takes_them() {
  let list = route_json(&at(ProjectPage::Review));
  assert_eq!(list["name"], json!("review"));
  assert_eq!(list["params"], json!({ "project": PROJECT }));
  let task = route_json(&at(ProjectPage::TaskReview(key("KON-12"))));
  assert_eq!(task["name"], json!("review"));
  assert_eq!(task["params"], json!({ "project": PROJECT, "key": "KON-12" }));
}
