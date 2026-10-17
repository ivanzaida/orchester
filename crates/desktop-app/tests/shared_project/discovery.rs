//! `orchester.toml` prefills the project's address and is contacted only after the person confirms it (`N9NFio`,
//! `oOGMT`): a file naming a project on a connected server is offered, and nothing asks for that project until the
//! person takes the address and links; a file naming a server this computer isn't connected to asks step by step and
//! sends nothing there at all. A linked project's *Write orchester.toml* is a person's, and unavailable with why when
//! the project's address is not on its server.

use std::fs;

use lurq::app::events::with_synthetic_input;
use orchester_store::ProjectDb;

use super::{KONTUR_REMOTE, harness, join, kontur_root, link_kontur, no_admin_endpoint, open_sharing, server};
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

#[test]
fn shared_project_discovery_is_written_for_a_linked_project_only_by_a_person() {
  let (server, kontur) = server();
  let mut window = harness::window(Some(KONTUR_REMOTE));
  let recorder = join(&window, &server, &kontur, ("ivan", "Ivan Zaida"));
  link_kontur(&window);
  let file = kontur_root(&window).join("orchester.toml");

  open_sharing(&mut window, "sharing-discovery-write");
  with_synthetic_input(|| window.press("sharing-discovery-write"));
  shows(&mut window, "sharing-discovery-write-person-only");
  assert!(!file.exists(), "an MCP client's press writes nothing");

  press(&mut window, "sharing-discovery-write");
  shows(&mut window, "sharing-discovery-file");
  assert_eq!(
    fs::read_to_string(&file).expect("orchester.toml"),
    format!("project = \"{}/kontur\"\n", recorder.url())
  );
  no_admin_endpoint(&recorder);
}

#[test]
fn shared_project_discovery_is_not_written_for_an_address_off_its_server() {
  let (server, kontur) = server();
  let mut window = harness::window(Some(KONTUR_REMOTE));
  let recorder = join(&window, &server, &kontur, ("ivan", "Ivan Zaida"));
  link_kontur(&window);
  // A link made before addresses were checked against their server.
  ProjectDb::open(&kontur_root(&window))
    .expect("Kontur's database")
    .record_project_address("https://orchester.elsewhere.example/kontur")
    .expect("the address");

  open_sharing(&mut window, "sharing-discovery-unavailable");
  let reason = window.text("sharing-discovery-unavailable");
  assert!(
    reason.contains("names this project at orchester.elsewhere.example"),
    "the reason names the address the server gives itself: {reason}"
  );
  press(&mut window, "sharing-discovery-write");
  window.pass();
  assert!(!kontur_root(&window).join("orchester.toml").exists());
  assert!(window.tree.get_element_by_id("sharing-discovery-error").is_none());
  // The address is longer than its box: a text that ends in an ellipsis there, not an input that runs past it.
  assert!(
    window
      .text("sharing-address-value")
      .ends_with("elsewhere.example/kontur")
  );
  assert!(
    window
      .tree
      .get_element_by_id_mut("sharing-address-value")
      .and_then(|element| element.as_text_input())
      .is_none()
  );
  no_admin_endpoint(&recorder);
}
