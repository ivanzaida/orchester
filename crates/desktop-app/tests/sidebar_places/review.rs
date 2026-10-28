//! The Review hub (`Hub pages` `ZRBS5`): the Merge queue and the Pull requests as its tabs, each its own route, so
//! a press on a tab is a step of the tab's history that Back and Forward retrace, and Review stays selected in the
//! sidebar on both.

use orchester_app::routes::{ProjectPage, Route};

use super::{
  overview, selected,
  support::{Window, project_id},
};

#[test]
fn sidebar_review_hub_tabs_keep_their_own_routes() {
  let mut window = Window::open();
  let navigation = window.navigation();
  navigation.go(overview(project_id()));
  window.until("the project to open", |window| window.texts("nav-review").is_some());
  window.press("nav-review");
  let queue = Route::Project(project_id(), ProjectPage::MergeQueue);
  window.until("the Review hub", |window| window.texts("review-hub").is_some());
  assert_eq!(navigation.route_now(), queue, "Review opens on its Merge queue");
  assert_eq!(window.text("review-hub-title"), "Review");
  let tabs = window.texts("review-hub-tabs").expect("the hub's tabs");
  assert!(
    tabs.first().is_some_and(|tab| tab == "Merge queue") && tabs.iter().any(|tab| tab == "Pull requests"),
    "{tabs:?}"
  );
  assert!(
    window.texts("screen-merge_queue").is_some(),
    "the Merge queue under the tabs"
  );
  window.press("review-hub-tab-pull_requests");
  let requests = Route::Project(project_id(), ProjectPage::PullRequests(Default::default()));
  window.until("the Pull requests tab", |window| {
    window.texts("screen-pull_requests").is_some()
  });
  assert_eq!(navigation.route_now(), requests);
  assert!(selected(&mut window, "nav-review"), "Review stays selected on its tabs");
  navigation.back();
  window.until("Back to the Merge queue", |window| {
    window.texts("screen-merge_queue").is_some()
  });
  assert_eq!(navigation.route_now(), queue);
  assert!(selected(&mut window, "nav-review"));
  navigation.forward();
  window.pass();
  assert_eq!(navigation.route_now(), requests, "Forward returns to the tab");
  // A link to a tab lands on it, in the hub.
  navigation.go(Route::parse(&queue.path()).expect("the tab's path reads"));
  window.until("the linked tab", |window| window.texts("screen-merge_queue").is_some());
  assert!(window.texts("review-hub").is_some());
}
