//! Review (`Review - v2` `j1Drb`): the sidebar's entry opens its list at `/review`, with Back and Forward, and stays
//! selected on a task's review and on the pages that were its tabs before (the Merge queue, the Pull requests, the
//! Merges), which keep their own routes and no longer have a hub's head or tabs.

use orchester_app::routes::{ProjectPage, Route};

use super::{
  overview, selected,
  support::{Window, project_id},
};

#[test]
fn sidebar_review_opens_its_list_and_history_retraces_it() {
  let mut window = Window::open();
  let navigation = window.navigation();
  let project = project_id();
  navigation.go(overview(project));
  window.until("the project to open", |window| window.texts("nav-review").is_some());
  window.press("nav-review");
  let list = Route::Project(project, ProjectPage::Review);
  window.until("Review", |window| window.texts("screen-review").is_some());
  assert_eq!(navigation.route_now(), list, "Review opens its list");
  assert!(selected(&mut window, "nav-review"));
  assert!(
    window.texts("review-hub").is_none() && window.texts("review-hub-tabs").is_none(),
    "Review is no hub of tabs"
  );
  let task = Route::parse(&format!("{}/KON-12", list.path())).expect("a task's review path reads");
  navigation.go(task.clone());
  window.until("a task's review", |window| {
    window
      .texts("screen-review")
      .is_some_and(|texts| texts[0].contains("KON-12"))
  });
  assert!(
    selected(&mut window, "nav-review"),
    "Review stays selected on a task's review"
  );
  navigation.back();
  window.until("Back to the list", |window| {
    window
      .texts("screen-review")
      .is_some_and(|texts| !texts[0].contains("KON-12"))
  });
  assert_eq!(navigation.route_now(), list);
  navigation.forward();
  window.pass();
  assert_eq!(navigation.route_now(), task, "Forward returns to the task's review");
}
