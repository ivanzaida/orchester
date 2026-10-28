//! What the design verification of N1 found, held in place: the key a first link keeps, the linked page's rows, the
//! projects an offline server's entry still lists, the end of *Connect a server* at a server already connected, *Try
//! again* when nothing answers, the token note after signing out, and the steps in their box with *Cancel* at its end.

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
    Route::Project(support::project_id(), ProjectPage::Settings(Default::default())),
    "settings-tab-sharing",
  );
  press(&mut window, "settings-tab-sharing");
  press(&mut window, "sharing-share-open");
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
  // *Team server* (`rsQ79` in `04b Sharing · shared` `TnUGj`): Address, Your role and Members, no Repository row.
  shows(&mut window, "sharing-role-value");
  let link = orchester_store::ProjectDb::open(&window.data.repo("repo-kontur"))
    .and_then(|db| db.sharing())
    .expect("the link")
    .expect("linked");
  assert_eq!(link.key.as_str(), "BIL", "linked under the server's key");
  assert!(
    window.tree.get_element_by_id("sharing-role-hint").is_none(),
    "the role has no line under it (`BsDal`)"
  );
  assert_eq!(
    window.text("sharing-role-value"),
    "Member",
    "the role in a word (`GgOpf`)"
  );
  let address = window.text("sharing-address-hint");
  assert!(address.starts_with("Synced "), "when it last synced: {address}");
  assert!(address.contains("connected"), "{address}");
  assert!(
    window.tree.get_element_by_id("sharing-repository").is_none(),
    "the design's Team server has no Repository row"
  );
  assert_eq!(window.text("sharing-leave-row-label"), "Leave the project");
  press(&mut window, "sharing-leave");
  shows(&mut window, "sharing-leave-question");
  // The server project by its name, once the server answered it.
  window.until("the server project's name in the question", |window| {
    window
      .texts("sharing-leave-question")
      .is_some_and(|texts| texts.join(" ").contains("Billing"))
  });
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
  window.until("the server's version", |window| {
    window
      .texts("orchester-server-header-facts")
      .is_some_and(|texts| texts.join(" ").contains("Orchester server "))
  });
  // The row's line names the checkout linked here (`quCVl` in `C6qnY`).
  let line = window.text("orchester-server-project-kontur-entry-hint");
  let checkout = line.rsplit("linked to ").next().unwrap_or_default();
  assert!(
    checkout.contains('…') && checkout.ends_with("repo-kontur") && checkout.chars().count() <= 40,
    "a long checkout path is cut in its middle: {line}"
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
  // The done *Address* step's line: the server's address, then *Change* (`Vn33B` in `MrVql`).
  let found = window.text("orchester-connect-found");
  assert!(recorder.url().ends_with(&found), "{found}");
  shows(&mut window, "orchester-connect-change");
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

/// The steps are the rows of one box, each as wide as it, and *Cancel* ends the row under it (`A8uyj`, `qXW3I` in
/// `MrVql`; Verify, attempt 3).
#[test]
fn orchester_design_connect_steps_are_rows_of_a_box_with_cancel_at_its_end() {
  let mut window = window(None);
  go(
    &mut window,
    Route::Orchester(OrchesterView::connect()),
    "orchester-connect-address",
  );
  let mut bounds = |id: &str| {
    window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  };
  let steps = bounds("orchester-connect-steps");
  let mut bottom = steps.y;
  for id in [
    "orchester-connect-step-address",
    "orchester-connect-step-compare",
    "orchester-connect-step-join",
    "orchester-connect-step-username",
  ] {
    let step = bounds(id);
    assert!(
      (step.width - steps.width).abs() < 0.5,
      "{id} spans the box: {step:?} {steps:?}"
    );
    assert!(step.y + 0.5 >= bottom, "{id} follows the step above: {step:?}");
    bottom = step.y + step.height;
  }
  assert!(bottom <= steps.y + steps.height + 0.5, "the steps stay in their box");
  let actions = bounds("orchester-connect-actions");
  let cancel = bounds("orchester-connect-cancel");
  assert!(
    (cancel.x + cancel.width - (actions.x + actions.width)).abs() < 0.5,
    "Cancel ends the row: {cancel:?} {actions:?}"
  );
  assert!(
    (actions.x + actions.width - (steps.x + steps.width)).abs() < 0.5,
    "the row under the box ends with it: {actions:?} {steps:?}"
  );
}
