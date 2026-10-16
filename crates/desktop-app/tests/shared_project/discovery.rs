//! `orchester.toml` prefills the project's address and is contacted only after the person confirms it (`N9NFio`,
//! `oOGMT`): a file naming a project on a connected server is offered, and nothing asks for that project until the
//! person takes the address and links; a file naming a server this computer isn't connected to asks step by step and
//! sends nothing there at all.

use std::fs;

use super::{KONTUR_REMOTE, harness, join, kontur_root, no_admin_endpoint, open_sharing, server};
use crate::harness::{Recorder, press, shows};

/// Whether `recorder` passed a request that looks the project `slug` up.
fn asked_for(recorder: &Recorder, slug: &str) -> bool {
  recorder
    .lines()
    .iter()
    .any(|line| line.contains(&format!("slug={slug}")))
}

#[test]
fn shared_project_discovery_is_contacted_only_after_the_person_confirms() {
  let (server, kontur) = server();
  let mut window = harness::window(Some(KONTUR_REMOTE));
  let recorder = join(&window, &server, &kontur, ("ivan", "Ivan Zaida"));
  let address = format!("{}/kontur", recorder.url());
  fs::write(
    kontur_root(&window).join("orchester.toml"),
    format!("project = \"{address}\"\n"),
  )
  .expect("orchester.toml");

  open_sharing(&mut window, "sharing-discovery");
  assert!(window.text("sharing-discovery").contains(&address));
  assert!(
    !asked_for(&recorder, "kontur"),
    "nothing asked before the person confirms"
  );
  press(&mut window, "sharing-discovery-use");
  assert_eq!(window.text("sharing-address"), address);
  assert!(!asked_for(&recorder, "kontur"), "taking the address asks nothing yet");
  press(&mut window, "sharing-link-button");
  shows(&mut window, "sharing-upload");
  assert!(asked_for(&recorder, "kontur"), "linking asks for the project");
  no_admin_endpoint(&recorder);
}

#[test]
fn shared_project_discovery_of_another_server_sends_nothing_there() {
  let (server, kontur) = server();
  let (elsewhere, _) = super::server();
  let watched = Recorder::start(elsewhere.address);
  let mut window = harness::window(Some(KONTUR_REMOTE));
  join(&window, &server, &kontur, ("ivan", "Ivan Zaida"));
  fs::write(
    kontur_root(&window).join("orchester.toml"),
    format!("project = \"{}/kontur\"\n", watched.url()),
  )
  .expect("orchester.toml");

  open_sharing(&mut window, "sharing-unconnected");
  assert!(window.text("sharing-unconnected-body").contains(&watched.url()));
  assert!(
    window
      .text("sharing-unconnected-step-1")
      .contains("compare its check code")
  );
  press(&mut window, "sharing-unconnected-not-now");
  window.until("the steps put away", |window| {
    window.tree.get_element_by_id("sharing-unconnected").is_none()
  });
  assert!(watched.lines().is_empty(), "nothing was sent to the file's server");
}
