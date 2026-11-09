//! *Publish for the team* from a version of the person's own that the team server did not number yet (ORC-837 review):
//! a refused version taken up again, and a version that waited and was taken back by *Cancel sending*, are published on
//! the team's version, which the form was never opened on: the draft says it is edited from v1, not from the person's
//! own v2 that is gone, so the version is sent and no one else's version is claimed to have been published meanwhile.
//!
//! `cargo test -p orchester-app --features test-support --test team_qa hub_unsent_publish`

use std::{
  thread,
  time::{Duration, Instant},
};

use orchester_api::test_support::team_server::TeamServer;
use orchester_app::test_support::Window;
use orchester_core::id::ResourceId;

use super::{
  hub_edit::{ARGUMENTS, PUBLISH, db, draft_of, on_pencil_until},
  team::State,
};

/// How many versions the stand-in holds of `pencil-design`, and the last one's body as text.
fn held(server: &TeamServer) -> (usize, String) {
  server
    .resources("kontur")
    .into_iter()
    .find(|(resource, _)| resource["name"] == "pencil-design")
    .map(|(_, versions)| (versions.len(), format!("{:?}", versions.last())))
    .expect("the resource")
}

/// Waits until the server numbered the version typed as `arguments` and the panel shows it as the team's, with no
/// notice that someone else published meanwhile. The app's monitor may not have noticed yet that a server that was
/// down is back: the pass that sends what waits is asked until it does.
fn until_sent(window: &mut Window, server: &TeamServer, (pencil, arguments): (ResourceId, &str)) {
  let root = window.data.repo("repo-kontur");
  let services = window.launch.services().expect("the session").clone();
  let deadline = Instant::now() + Duration::from_secs(20);
  while held(server).0 < 2 {
    window
      .data
      .runtime
      .block_on(services.servers.send_published(&root))
      .expect("sent");
    assert!(Instant::now() < deadline, "the server never numbered the version");
    thread::sleep(Duration::from_millis(100));
  }
  assert!(held(server).1.contains(arguments), "{}", held(server).1);
  window.until("the panel on the team's v2", |window| {
    window.text("team-hub-chip-team") == "Team · v2"
      && db(window).waiting_publishes().expect("outbox").is_empty()
      && draft_of(window, pencil).is_none()
  });
  assert!(window.texts("team-hub-edit-refused-title").is_none());
}

#[test]
fn team_qa_hub_edit_unsent_publish_a_refused_form_is_sent_on_the_teams_version_once_the_reason_is_fixed() {
  let (mut window, server, (_, pencil)) = on_pencil_until(
    ("team-edit-rejected", State::TeamEditRejected),
    "team-hub-edit-refused-title",
  );
  server.accept_content("kontur");
  window.set(ARGUMENTS, "--fixed");
  window.until("the fix to be kept", |window| {
    draft_of(window, pencil) == Some(vec!["--fixed".to_owned()])
  });
  window.press(PUBLISH);

  until_sent(&mut window, &server, (pencil, "--fixed"));
}

#[test]
fn team_qa_hub_edit_unsent_publish_cancel_sending_then_an_edit_is_sent_on_the_teams_version() {
  let (mut window, server, (_, pencil)) = on_pencil_until(
    ("team-edit-waiting", State::TeamEditWaiting),
    "team-hub-edit-waiting-title",
  );
  window.press("team-hub-edit-cancel");
  window.until("the form", |window| window.texts(PUBLISH).is_some());
  server.set_down(false);
  window.set(ARGUMENTS, "--again");
  window.until("the edit to be kept", |window| {
    draft_of(window, pencil) == Some(vec!["--again".to_owned()])
  });
  window.press(PUBLISH);

  until_sent(&mut window, &server, (pencil, "--again"));
}
