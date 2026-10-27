//! A team server and *Connect a server* open as pages under Settings › Orchester Cloud (`05c` `C6qnY`, `05b`
//! `MrVql`), each with its crumb back to the tab; each is an entry of the tab's history: Back returns to the one
//! before, Forward comes back.

use orchester_app::routes::{OrchesterView, Route};

use super::{
  harness::{go, press, shows, window},
  joined_through_recorder,
};

fn now(window: &super::support::Window) -> Route {
  window.navigation().route_now()
}

fn back(window: &mut super::support::Window) {
  window.navigation().back();
  window.pass();
}

fn forward(window: &mut super::support::Window) {
  window.navigation().forward();
  window.pass();
}

#[test]
fn orchester_server_and_connect_pages_open_with_crumbs_and_are_history() {
  let mut window = window(None);
  let (_server, _recorder) = joined_through_recorder(&window);
  let services = window.launch.services().expect("services").clone();
  let server = services.servers.entries().expect("entries").remove(0).stored.id;
  let row = format!("orchester-row-{server}-open");
  let cloud = Route::Orchester(OrchesterView::default());
  go(&mut window, cloud.clone(), &row);
  press(&mut window, &row);
  let chose_server = Route::Orchester(OrchesterView::server(server));
  assert_eq!(now(&window), chose_server);
  shows(&mut window, "orchester-server-details");
  assert!(window.text("orchester-server-crumb").contains("Orchester Cloud"));
  press(&mut window, "orchester-server-crumb");
  assert_eq!(now(&window), cloud);
  press(&mut window, "orchester-connect-server");
  let connect = Route::Orchester(OrchesterView::connect());
  assert_eq!(now(&window), connect);
  shows(&mut window, "orchester-connect");
  // *Connect a server* is a page under the tab, with Settings' head and tabs above it.
  shows(&mut window, "settings-tab-orchester");

  back(&mut window);
  assert_eq!(now(&window), cloud);
  back(&mut window);
  assert_eq!(now(&window), chose_server);
  back(&mut window);
  assert_eq!(now(&window), cloud);
  forward(&mut window);
  assert_eq!(now(&window), chose_server);
  forward(&mut window);
  forward(&mut window);
  assert_eq!(now(&window), connect);
  press(&mut window, "orchester-connect-crumb");
  assert_eq!(now(&window), cloud);
}
