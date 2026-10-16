//! Members are read-only (`L5o9Xy`, `N9NFio`): a member is told to ask an owner and has no Manage; an owner's
//! *Manage members and invites* opens the project's page in the server admin (recorded, never a browser); while the
//! server cannot be reached the members show as last read, and Manage is unavailable. No member or role control exists.

use super::{KONTUR_REMOTE, harness, join, link_kontur, no_admin_endpoint, open_sharing, server};
use crate::harness::{press, shows};

#[test]
fn shared_project_members_are_read_only_and_manage_opens_the_admin() {
  let (server, kontur) = server();
  let mut window = harness::window(Some(KONTUR_REMOTE));
  let recorder = join(&window, &server, &kontur, ("ivan", "Ivan Zaida"));
  link_kontur(&window);

  open_sharing(&mut window, "sharing-member-ivan");
  shows(&mut window, "sharing-member-alice");
  assert_eq!(window.text("sharing-member-ivan-role"), "Member");
  assert_eq!(window.text("sharing-member-alice-role"), "Owner");
  assert_eq!(window.text("sharing-member-ivan-you"), "· you");
  assert!(window.tree.get_element_by_id("sharing-member-alice-you").is_none());
  assert!(
    window
      .text("sharing-members-intro-text")
      .contains("To add someone, ask an owner: Alice Moreau."),
  );
  assert!(
    window.tree.get_element_by_id("sharing-members-manage").is_none(),
    "a member manages nothing"
  );

  // Made an owner in the server admin: the app learns it at its next read.
  server.set_role(&kontur, "ivan", "owner");
  window.launch.services().expect("services").servers.sync_now();
  shows(&mut window, "sharing-members-manage");
  press(&mut window, "sharing-members-manage");
  let links = window.launch.services().expect("services").links.recorded();
  assert_eq!(links, [format!("{}/admin/projects/kontur", recorder.url())]);
  assert!(
    window
      .text("sharing-members-manage-note")
      .contains("/admin/projects/kontur")
  );

  // The server goes away: the members stay as last read, and Manage waits for it.
  recorder.go_down();
  window
    .launch
    .services()
    .expect("services")
    .servers
    .retry_now(super::entry(&window).stored.id);
  window.until("the members as last read", |window| {
    window
      .texts("sharing-members-intro-text")
      .is_some_and(|texts| texts.join(" ").contains("when Kontur team was last reached"))
  });
  shows(&mut window, "sharing-member-alice");
  press(&mut window, "sharing-members-manage");
  assert_eq!(
    window.launch.services().expect("services").links.recorded().len(),
    1,
    "Manage is unavailable while the server can't be reached"
  );
  no_admin_endpoint(&recorder);
}
