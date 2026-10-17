//! Choosing the cloud account, a team server or *Connect a server* on Settings › Orchester is an entry of the
//! tab's history: Back returns to the one before, Forward comes back.

use orchester_app::routes::{OrchesterView, Route};

use super::{
  harness::{go, press, window},
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
fn orchester_chosen_servers_and_panels_are_history() {
  let mut window = window(None);
  let (_server, _recorder) = joined_through_recorder(&window);
  let services = window.launch.services().expect("services").clone();
  let server = services.servers.entries().expect("entries").remove(0).stored.id;
  let row = format!("orchester-row-{server}");
  let cloud = Route::Orchester(OrchesterView::default());
  go(&mut window, cloud.clone(), &row);
  press(&mut window, &row);
  let chose_server = Route::Orchester(OrchesterView::server(server));
  assert_eq!(now(&window), chose_server);
  press(&mut window, "orchester-row-cloud");
  assert_eq!(now(&window), cloud);
  press(&mut window, "settings-connect-server");
  let connect = Route::Orchester(OrchesterView::connect());
  assert_eq!(now(&window), connect);

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
}
