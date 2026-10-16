//! What the design verification of N1 found, held in place: the key a first link keeps, the linked page's rows, the
//! projects an offline server's entry still lists, the end of *Connect a server* at a server already connected, *Try
//! again* when nothing answers, and the token note after signing out.

use std::net::TcpListener;

use orchester_app::routes::{OrchesterView, ProjectPage, Route};
use orchester_workspace::sharing::Connection;

use super::{
  KONTUR_REMOTE,
  harness::{go, press, shows, window},
  joined_through_recorder, link_kontur, support,
};

#[test]
fn orchester_design_link_takes_the_server_projects_key() {
  let mut window = window(Some(KONTUR_REMOTE));
  let (_server, _recorder) = joined_through_recorder(&window);
  go(
    &mut window,
    Route::Project(support::project_id(), ProjectPage::Settings),
    "settings-tab-sharing",
  );
  press(&mut window, "settings-tab-sharing");
  shows(&mut window, "sharing-project-billing-option");
  assert!(
    window.text("sharing-project-billing-option").contains("Empty"),
    "an empty project says so: {}",
    window.text("sharing-project-billing-option")
  );
  press(&mut window, "sharing-project-billing");
  press(&mut window, "sharing-link-button");
  shows(&mut window, "sharing-mismatch");
  press(&mut window, "sharing-link-anyway");
  shows(&mut window, "sharing-upload");
  let question = window.text("sharing-upload");
  assert!(
    question.contains("KON-1 becomes BIL-1"),
    "the confirmation names the key the tasks take: {question}"
  );
  press(&mut window, "sharing-link-confirm");
  shows(&mut window, "sharing-shared-detail");
  let detail = window.text("sharing-shared-detail");
  assert!(detail.starts_with("BIL ·"), "linked under the server's key: {detail}");
  assert!(detail.contains("synced"), "when it last synced: {detail}");
  assert!(window.text("sharing-address-state").contains("connected"));
  assert!(
    window
      .text("sharing-make-local-description")
      .contains("BIL stays the same for everyone else")
  );
  window.until("the repository compared", |window| {
    window
      .texts("sharing-repository-state")
      .is_some_and(|texts| texts.join(" ").contains("doesn't match"))
  });
  assert!(
    window
      .text("sharing-task-list-description")
      .starts_with("Kontur's tasks live in Billing on"),
    "the server project by its name: {}",
    window.text("sharing-task-list-description")
  );
  assert_eq!(window.text("sharing-leave-row-label"), "Leave Billing");
  assert!(
    window
      .text("sharing-repository-value")
      .contains("github.com/acme/billing"),
    "the project's hint: {}",
    window.text("sharing-repository-value")
  );
}

#[test]
fn orchester_design_offline_server_lists_the_projects_linked_here() {
  let mut window = window(Some(KONTUR_REMOTE));
  let (_server, recorder) = joined_through_recorder(&window);
  let services = window.launch.services().expect("services").clone();
  let entry = link_kontur(&window);
  go(
    &mut window,
    Route::Orchester(OrchesterView::server(entry.stored.id)),
    "orchester-server-project-billing-link",
  );
  recorder.go_down();
  // The monitor asks a connected server once a minute; *Try now* asks at once.
  services.servers.retry_now(entry.stored.id);
  window.until("the server offline", |_| {
    services.servers.entries().is_ok_and(|entries| {
      matches!(
        entries[0].connection,
        Some(Connection::Reconnecting { .. } | Connection::Offline { .. })
      )
    })
  });
  window.until("the offline list", |window| {
    window
      .texts("orchester-server-projects-unread")
      .is_some_and(|texts| texts.join(" ").contains("can't be reached"))
  });
  shows(&mut window, "orchester-server-project-kontur");
  assert!(
    window
      .tree
      .get_element_by_id("orchester-server-projects-reading")
      .is_none(),
    "no reading line while the server is offline"
  );
}

#[test]
fn orchester_design_connect_ends_at_a_server_already_connected() {
  let mut window = window(None);
  let (_server, recorder) = joined_through_recorder(&window);
  go(
    &mut window,
    Route::Orchester(OrchesterView::connect()),
    "orchester-connect-address",
  );
  window.set("orchester-connect-address", &recorder.url());
  press(&mut window, "orchester-connect-continue");
  shows(&mut window, "orchester-connect-already");
  for later in [
    "orchester-connect-step-compare",
    "orchester-connect-step-join",
    "orchester-connect-step-username",
  ] {
    assert!(
      window.tree.get_element_by_id(later).is_none(),
      "{later} is not offered for a server already connected"
    );
  }
}

#[test]
fn orchester_design_connect_tries_again_when_nothing_answers() {
  let closed = {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listen");
    listener.local_addr().expect("address").port()
  };
  let mut window = window(None);
  go(
    &mut window,
    Route::Orchester(OrchesterView::connect()),
    "orchester-connect-address",
  );
  shows(&mut window, "orchester-connect-address-hint");
  window.set("orchester-connect-address", &format!("http://localhost:{closed}"));
  press(&mut window, "orchester-connect-continue");
  shows(&mut window, "orchester-connect-address-error");
  assert_eq!(window.text("orchester-connect-continue"), "Try again");
  assert!(
    window
      .tree
      .get_element_by_id("orchester-connect-address-hint")
      .is_none(),
    "the hint gives way to the error"
  );
}
