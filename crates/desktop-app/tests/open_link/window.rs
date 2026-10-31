//! The window that takes a link: an invite the account's own listing has opens Settings › Orchester at its row, which
//! keeps its own *Accept* and *Decline*; any other link, an invite not listed included, only shows *This link isn't
//! for anything here* over the screen; signed out, the gate shows first and the invite opens after the sign-in.

use orchester_app::{
  open_link::{OpenLink, host::NOTICE_ID},
  routes::{OrchesterPanel, OrchesterView, Route},
};
use orchester_core::id::InviteId;

use super::{
  INVITE,
  support::{
    Data, Window,
    signed_in::{
      self,
      account_cloud::{EMAIL, PASSWORD},
    },
  },
};

/// The team server the stand-in cloud's invites are from.
const SERVER: &str = "0192f3a4-0000-7000-8000-000000000001";
/// An invite the cloud does not list.
const OTHER: &str = "0192f3a4-0000-7000-8000-0000000000ff";

fn invite() -> InviteId {
  INVITE.parse().expect("an invite id")
}

/// The window, signed in, with the stand-in cloud listing the invite [`INVITE`] to the server alone.
fn with_invite(window: Window) -> Window {
  window.data.cloud.name_server(SERVER, "https://kontur.example", None);
  window.data.cloud.invite(INVITE, None);
  window.launch.services().expect("services").links.record_instead();
  window
}

fn push(window: &Window, link: OpenLink) {
  window.launch.services().expect("services").opened_links.push(link);
}

fn at_invite() -> Route {
  Route::Orchester(OrchesterView {
    panel: OrchesterPanel::Invites,
    at: Some(invite()),
    ..OrchesterView::default()
  })
}

fn shows(window: &mut Window, id: &str) {
  window.until(id, |window| window.tree.get_element_by_id(id).is_some());
}

fn has(window: &mut Window, id: &str) -> bool {
  window.tree.get_element_by_id(id).is_some()
}

#[test]
fn open_link_window_opens_a_listed_invite_at_its_row() {
  let mut window = with_invite(Window::open());
  let before = window.navigation().route_now();
  push(&window, OpenLink::Invite(invite()));
  window.until("Settings › Orchester at the invite", |window| {
    window.navigation().route_now() == at_invite()
  });
  assert_ne!(before, at_invite());
  let row = format!("orchester-invite-{INVITE}");
  shows(&mut window, &row);
  assert!(
    window.text(&row).contains("Alice Moreau"),
    "the row's words are the cloud's listing: {}",
    window.text(&row)
  );
  assert!(
    has(&mut window, &format!("{row}-accept")),
    "the row keeps its own Accept"
  );
  assert!(!has(&mut window, NOTICE_ID));
  assert!(
    window.data.cloud.declined().is_empty() && window.data.cloud.tokens_for(INVITE) == 0,
    "a link accepts and declines nothing"
  );
}

#[test]
fn open_link_window_says_an_unlisted_invite_is_for_nothing_here() {
  let mut window = with_invite(Window::open());
  let before = window.navigation().route_now();
  push(&window, OpenLink::Invite(OTHER.parse().expect("an id")));
  shows(&mut window, NOTICE_ID);
  assert_eq!(
    window.text(&format!("{NOTICE_ID}-title")),
    "This link isn't for anything here"
  );
  assert_eq!(window.navigation().route_now(), before, "nothing else happens");
}

#[test]
fn open_link_window_says_an_unknown_link_is_for_nothing_here_until_dismissed() {
  let mut window = with_invite(Window::open());
  let before = window.navigation().route_now();
  push(&window, OpenLink::Unknown);
  shows(&mut window, NOTICE_ID);
  assert_eq!(window.navigation().route_now(), before);
  window.press(&format!("{NOTICE_ID}-dismiss"));
  window.until("the notice gone", |window| {
    window.tree.get_element_by_id(NOTICE_ID).is_none()
  });
  push(&window, OpenLink::Open);
  window.pass();
  window.pass();
  assert!(!has(&mut window, NOTICE_ID), "open only brings the window forward");
  assert_eq!(window.navigation().route_now(), before);
}

#[test]
fn open_link_window_signed_out_opens_the_invite_after_the_gate() {
  let data = Data::new();
  let launch = data.launch_signed_out();
  let mut window = with_invite(Window::of(data, launch));
  push(&window, OpenLink::Invite(invite()));
  shows(&mut window, "screen-sign-in-gate");
  for _ in 0..20 {
    window.tree.tick_futures();
    window.pass();
  }
  assert!(!has(&mut window, NOTICE_ID), "the gate comes first");
  assert!(has(&mut window, "screen-sign-in-gate"));
  // The gate on the stand-in cloud, as *Advanced · cloud address* sets it, and a person's sign-in.
  let services = window.launch.services().expect("services").clone();
  let address = signed_in::cloud_address(&window.data.cloud);
  window
    .data
    .runtime
    .block_on(services.cloud.set_address(Some(address)))
    .expect("a test cloud");
  window.until("the gate on the test cloud", |window| {
    window
      .texts("gate-footer-cloud")
      .is_some_and(|texts| texts.join("").ends_with("· test cloud"))
  });
  shows(&mut window, "gate-email");
  window.set("gate-email", EMAIL);
  window.set("gate-password", PASSWORD);
  window.press("gate-sign-in");
  window.until("the invite after the sign-in", |window| {
    window.tree.get_element_by_id("screen-sign-in-gate").is_none() && window.navigation().route_now() == at_invite()
  });
  shows(&mut window, &format!("orchester-invite-{INVITE}"));
}
