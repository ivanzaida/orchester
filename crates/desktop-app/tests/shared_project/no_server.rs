//! Sharing when this computer's list of servers no longer has the project's server (linked from another computer's
//! list, or the server was removed from Settings › Orchester): the page names the server by its address's host, says
//! it is no longer in Settings › Orchester, and *Write orchester.toml* and *Leave* are unavailable with why before any
//! press (`Manage` in `q8DIyg`); the members show as last read, with when, and the person among them, also once they
//! are no longer one.

use std::collections::HashMap;

use orchester_core::{
  id::{ProjectId, ServerId, UserId},
  project::ProjectKey,
  sharing::MemberRole,
  time::Timestamp,
};
use orchester_store::{ProjectDb, SharedMember, SharingLink};

use super::{KONTUR_REMOTE, harness, kontur_root, open_sharing, support::Window};
use crate::harness::shows;

const HOST: &str = "orchester.kontur.lan";

/// Links the window's Kontur to a server the window's list doesn't have, with the members as read by `me` (Ivan Zaida)
/// an hour ago; `removed` once Ivan is no longer a member.
fn linked_without_server(window: &Window, me: UserId, removed: Option<Timestamp>) {
  let mut db = ProjectDb::open(&kontur_root(window)).expect("Kontur's database");
  let link = SharingLink {
    server: ServerId::generate(),
    project: ProjectId::generate(),
    key: ProjectKey::new("KON").expect("key"),
    role: Some(MemberRole::Member),
    joined_at: Timestamp::now(),
    cursor: 0,
    last_synced_at: Timestamp::now(),
    removed_at: removed,
    address: Some(format!("https://{HOST}/kontur")),
  };
  db.start_sharing(&link, &[], (&[], &HashMap::new())).expect("shared");
  let member = |user, (name, username, role)| SharedMember {
    user,
    display_name: String::from(name),
    username: String::from(username),
    role,
    added_at: Timestamp::now(),
  };
  let members = [
    member(UserId::generate(), ("Alice Moreau", "alice", MemberRole::Owner)),
    member(me, ("Ivan Zaida", "ivan", MemberRole::Member)),
  ];
  db.record_shared_members(&members, Timestamp::now(), Some(me))
    .expect("members read once");
}

#[test]
fn shared_project_without_its_server_says_so_and_waits_for_it() {
  let mut window = harness::window(Some(KONTUR_REMOTE));
  linked_without_server(&window, UserId::generate(), None);
  open_sharing(&mut window, "sharing-discovery-write");

  let address = window.text("sharing-address-hint");
  assert!(
    address.ends_with(&format!("{HOST} · no longer in Settings › Orchester")),
    "{address}"
  );
  let page = window.text("project-settings-sharing");
  assert!(!page.contains("a server no longer"), "{page}");

  // Write orchester.toml says why before any press, and a press writes nothing.
  assert!(
    window
      .text("sharing-discovery-unavailable")
      .contains("isn't in Settings › Orchester")
  );
  window.press("sharing-discovery-write");
  window.pass();
  assert!(!kontur_root(&window).join("orchester.toml").exists());
  assert!(window.tree.get_element_by_id("sharing-discovery-error").is_none());

  // Leave is done on the server: unavailable, with why, and its row doesn't say this computer stays connected.
  assert_eq!(
    window.text("sharing-leave-unavailable"),
    format!("Leave works once this computer is signed in to {HOST} again.")
  );
  assert!(!window.text("sharing-leave-row-hint").contains("stays connected"));
  window.press("sharing-leave");
  window.pass();
  assert!(window.tree.get_element_by_id("sharing-leave-question").is_none());

  shows(&mut window, "sharing-member-ivan-row");
  assert_eq!(window.text("sharing-members-count"), "2");
  assert!(
    window.text("sharing-members-hint").contains(" · as of "),
    "a list not read now says when it was"
  );
}

#[test]
fn shared_project_members_name_the_person_once_signed_out_and_removed() {
  let mut window = harness::window(Some(KONTUR_REMOTE));
  let me = UserId::generate();
  linked_without_server(&window, me, Some(Timestamp::now()));
  open_sharing(&mut window, "sharing-member-ivan-row");
  assert!(
    window.text("sharing-member-ivan-hint").contains("· you, until "),
    "the list knows which member the person was"
  );
  assert!(!window.text("sharing-member-alice-hint").contains("you"));
}
