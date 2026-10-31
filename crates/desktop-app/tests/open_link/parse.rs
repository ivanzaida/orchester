//! What a link asks for: the two forms, and everything else as no link of Orchester's.

use std::ffi::OsString;

use orchester_app::open_link::{ARGUMENT, MAX_LINE, OpenLink, argument};

use super::INVITE;

#[test]
fn open_link_parse_reads_an_invite_and_open() {
  let invite = INVITE.parse().expect("an invite id");
  assert_eq!(
    OpenLink::parse(&format!("orchester://invite/{INVITE}")),
    OpenLink::Invite(invite)
  );
  assert_eq!(
    OpenLink::parse(&format!("ORCHESTER://invite/{}", INVITE.to_uppercase())),
    OpenLink::Invite(invite),
    "the scheme without regard to case, and the id in either case"
  );
  assert_eq!(OpenLink::parse("orchester://open"), OpenLink::Open);
  assert_eq!(OpenLink::parse("Orchester://open"), OpenLink::Open);
}

#[test]
fn open_link_parse_refuses_every_other_form() {
  let simple = INVITE.replace('-', "");
  let refused = [
    String::new(),
    "orchester://".to_owned(),
    "orchester:".to_owned(),
    "orchester:open".to_owned(),
    "orchester:/open".to_owned(),
    "orchester://open/".to_owned(),
    "orchester://open?from=mail".to_owned(),
    "orchester://open#top".to_owned(),
    "orchester://OPEN".to_owned(),
    " orchester://open".to_owned(),
    "orchester://open ".to_owned(),
    "orchester://invite".to_owned(),
    "orchester://invite/".to_owned(),
    "orchester://invite/not-an-id".to_owned(),
    format!("orchester://invite/{simple}"),
    format!("orchester://invite/{{{INVITE}}}"),
    format!("orchester://invite/urn:uuid:{INVITE}"),
    format!("orchester://invite/{INVITE}/"),
    format!("orchester://invite/{INVITE}?accept=1"),
    format!("orchester://invite/{INVITE}#accept"),
    format!("orchester://invite/{INVITE}/accept"),
    format!("orchester://Invite/{INVITE}"),
    format!("orchester://invite//{INVITE}"),
    format!("orchester://server/{INVITE}"),
    format!("orchester://project/{INVITE}"),
    format!("orchester://invite/{INVITE}\n"),
    format!("https://orchester.cloud/invite/{INVITE}"),
    format!("orchestra://invite/{INVITE}"),
    format!("orchester://open{}", "x".repeat(MAX_LINE)),
    "orchester://ореn".to_owned(),
  ];
  for link in refused {
    assert_eq!(OpenLink::parse(&link), OpenLink::Unknown, "{link:?}");
  }
}

#[test]
fn open_link_parse_logs_the_form_alone() {
  let link = OpenLink::parse(&format!("orchester://invite/{INVITE}"));
  assert_eq!(link.form(), "orchester://invite/…");
  assert!(!link.form().contains(INVITE));
  assert_eq!(OpenLink::Open.form(), "orchester://open");
  assert_eq!(OpenLink::Unknown.form(), "unknown");
}

#[test]
fn open_link_argument_is_the_text_after_the_switch() {
  let args = |args: &[&str]| args.iter().map(OsString::from).collect::<Vec<_>>();
  assert_eq!(
    argument(args(&["orchester.exe", ARGUMENT, "orchester://open"])),
    Some("orchester://open".to_owned())
  );
  assert_eq!(argument(args(&["orchester.exe"])), None);
  assert_eq!(
    argument(args(&["orchester.exe", "orchester://open"])),
    None,
    "a link only after the switch"
  );
  assert_eq!(
    argument(args(&["orchester.exe", ARGUMENT])),
    Some(String::new()),
    "a switch with nothing after it is an empty link"
  );
}
