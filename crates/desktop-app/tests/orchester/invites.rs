//! An invite is a row of *Invites* with *Accept* and *Decline* at its end (`YwJ2b` in `ZUuMT`); *Decline* asks in place
//! of the row's actions (`DqgtJ`), *Keep it* takes the question back, and a person's *Decline* ends the invite.

use super::{
  account_cloud::{AccountCloud, EMAIL},
  harness::{press, shows},
  server_support::TestServer,
  signed_in,
};

#[test]
fn orchester_invite_row_declines_in_place_after_asking() {
  let cloud = AccountCloud::start();
  let server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let audience = server.link_cloud(&cloud.root_key, &cloud.key_set);
  cloud.name_server(&audience, &server.url(), None);
  let invite = server.email_invite(EMAIL);
  cloud.invite(&invite, None);
  let mut window = signed_in(&cloud);
  let card = format!("orchester-invite-{invite}");
  shows(&mut window, &format!("{card}-accept"));
  // Both answers sit in the invite's row, inside the group's box.
  for answer in ["accept", "decline"] {
    let inside = window
      .texts(&format!("{card}-entry-row"))
      .is_some_and(|texts| texts.iter().any(|text| text.eq_ignore_ascii_case(answer)));
    assert!(inside, "{answer} in the row");
  }
  assert!(window.text("orchester-invites-meta").contains("1 waiting"));

  press(&mut window, &format!("{card}-decline"));
  shows(&mut window, &format!("{card}-decline-question"));
  assert!(
    window.tree.get_element_by_id(&format!("{card}-accept")).is_none(),
    "the question takes the actions' place"
  );
  assert!(
    window
      .text(&format!("{card}-decline-question-title"))
      .starts_with("Decline the invite to")
  );
  press(&mut window, &format!("{card}-keep"));
  shows(&mut window, &format!("{card}-accept"));
  assert!(cloud.declined().is_empty(), "kept");

  press(&mut window, &format!("{card}-decline"));
  press(&mut window, &format!("{card}-decline-confirm"));
  window.until("the invite declined", |_| !cloud.declined().is_empty());
  shows(&mut window, "orchester-invites-none-row");
  assert!(window.tree.get_element_by_id(&card).is_none(), "the invite is gone");
}
