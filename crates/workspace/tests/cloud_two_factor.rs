//! Two-factor authentication on this computer (`docs/cloud-api.md`, "Two-factor authentication"): the password step
//! answers a challenge that a code from the app or a recovery code finishes, and nothing is kept before it does; a
//! wrong code, an ended challenge and too many codes are the cloud's refusals; few recovery codes left are said after
//! a sign-in with one; an accept refused for want of a code leaves the invite open; and signing out to sign in again
//! with a code is recorded as its own reason, with the invite's name.
//!
//! `cargo test -p orchester-workspace --test cloud_two_factor two_factor_`

#[path = "sharing_support/account_cloud.rs"]
mod account_cloud;
#[path = "sharing_support/desktop.rs"]
mod desktop;
#[path = "server_support/mod.rs"]
mod server_support;
#[path = "sharing_support/project.rs"]
mod support;

use std::time::{Duration, Instant};

use account_cloud::{AccountCloud, EMAIL, OTHER_EMAIL, PASSWORD, RECOVERY_CODES};
use desktop::{Desktop, linked_server};
use orchester_api::{Credential, cloud::CloudErrorCode};
use orchester_core::{secret::SecretVault, time::Timestamp};
use orchester_store::{AcceptedInvite, CloudSignInEnd};
use orchester_workspace::sharing::{
  CloudJoined,
  cloud::{CHALLENGE_LIFE, Challenge, CloudError, CodeKind, FEW_RECOVERY_CODES, SignIn, SignInStep},
};

/// Signs `desktop` in to `cloud` with the password; the challenge it answers.
fn password_step(desktop: &Desktop) -> Challenge {
  let step = desktop.block_on(desktop.cloud.sign_in(EMAIL.into(), Credential::new(PASSWORD.into())));
  match step {
    Ok(SignInStep::Code(challenge)) => challenge,
    other => panic!("the password answers a challenge: {other:?}"),
  }
}

fn finish(desktop: &Desktop, challenge: &Challenge, (code, kind): (&str, CodeKind)) -> Result<u32, CloudError> {
  desktop
    .block_on(
      desktop
        .cloud
        .finish_sign_in(challenge, (Credential::new(code.to_owned()), kind)),
    )
    .map(|signed_in| signed_in.recovery_codes_left)
}

#[test]
fn two_factor_sign_in_takes_a_code_after_the_password() {
  let cloud = AccountCloud::start();
  cloud.require_codes();
  let desktop = Desktop::new(&cloud.url());
  let challenge = password_step(&desktop);
  assert_eq!(challenge.email(), EMAIL);
  assert!(
    !format!("{challenge:?}").contains("orcm_"),
    "the challenge stays out of Debug"
  );
  assert!(desktop.cloud.account().expect("account").is_none(), "nothing kept yet");
  assert!(!desktop.cloud.is_signed_in());

  let wrong = finish(&desktop, &challenge, ("000000", CodeKind::App)).expect_err("a wrong code");
  assert_eq!(wrong.cloud_code(), Some(CloudErrorCode::InvalidCode));
  assert!(!desktop.cloud.is_signed_in(), "a wrong code signs nothing in");

  let left = finish(&desktop, &challenge, (&cloud.next_code(), CodeKind::App)).expect("the app's code");
  assert_eq!(left, 10);
  assert!(desktop.cloud.is_signed_in());
  assert_eq!(
    desktop.cloud.account().expect("account").expect("signed in").email,
    EMAIL
  );
  assert!(desktop.vault.read(&desktop.device_key()).is_ok(), "the token is kept");
  assert_eq!(
    desktop.cloud.few_recovery_codes(),
    None,
    "an app's code says nothing of them"
  );
  let two_factor = desktop.block_on(desktop.cloud.two_factor()).expect("two-factor");
  assert!(two_factor.enabled && two_factor.this_session);
}

#[test]
fn two_factor_a_recovery_code_signs_in_and_says_when_few_are_left() {
  let cloud = AccountCloud::start();
  cloud.require_codes();
  cloud.use_recovery_codes(10 - usize::try_from(FEW_RECOVERY_CODES).expect("a few"));
  let desktop = Desktop::new(&cloud.url());
  let challenge = password_step(&desktop);
  let spaced = RECOVERY_CODES[9].replace('-', " ").to_uppercase();
  let left = finish(&desktop, &challenge, (&spaced, CodeKind::Recovery)).expect("a recovery code");
  assert_eq!(left, FEW_RECOVERY_CODES - 1);
  assert_eq!(desktop.cloud.few_recovery_codes(), Some(FEW_RECOVERY_CODES - 1));

  desktop.block_on(desktop.cloud.sign_out()).expect("signed out");
  assert_eq!(
    desktop.cloud.few_recovery_codes(),
    None,
    "a new sign-in starts without it"
  );
}

#[test]
fn two_factor_an_ended_challenge_and_too_many_codes_are_refused() {
  let cloud = AccountCloud::start();
  cloud.require_codes();
  let desktop = Desktop::new(&cloud.url());
  let challenge = password_step(&desktop);
  cloud.end_challenges();
  let ended = finish(&desktop, &challenge, (&cloud.next_code(), CodeKind::App)).expect_err("ended");
  assert_eq!(ended.cloud_code(), Some(CloudErrorCode::ChallengeExpired));

  let challenge = password_step(&desktop);
  cloud.limit_codes(true);
  let limited = finish(&desktop, &challenge, (&cloud.next_code(), CodeKind::App)).expect_err("limited");
  assert_eq!(limited.cloud_code(), Some(CloudErrorCode::RateLimited));
  assert!(limited.retry_after().is_some(), "the cloud says how long");
  assert!(!desktop.cloud.is_signed_in());
}

#[test]
fn two_factor_an_accept_without_a_code_leaves_the_invite_open() {
  let cloud = AccountCloud::start();
  let (server, audience) = linked_server(&cloud);
  let invite = server.email_invite(EMAIL);
  let desktop = Desktop::signed_in(&cloud);
  cloud.name_server(&audience, &server.url(), None);
  cloud.invite(&invite, None);
  cloud.session_without_two_factor();

  let listed = desktop.block_on(desktop.cloud.invites()).expect("invites");
  assert!(listed[0].needs_two_factor, "the cloud says this computer needs a code");
  let refused = desktop.block_on(desktop.cloud.accept(&listed[0])).expect_err("refused");
  assert!(matches!(refused, CloudError::TwoFactorSession), "{refused:?}");
  assert_eq!(cloud.tokens_for(&invite), 0, "no token accepted it");
  assert!(
    desktop
      .block_on(desktop.cloud.accepted_invites())
      .expect("kept")
      .is_empty(),
    "nothing kept to ask again"
  );
  assert_eq!(desktop.block_on(desktop.cloud.invites()).expect("invites").len(), 1);

  cloud.two_factor_off();
  let refused = desktop.block_on(desktop.cloud.accept(&listed[0])).expect_err("refused");
  assert!(matches!(refused, CloudError::TwoFactorOff), "{refused:?}");
}

#[test]
fn two_factor_signing_out_for_a_code_names_the_invite() {
  let cloud = AccountCloud::start();
  let desktop = Desktop::signed_in(&cloud);
  desktop
    .block_on(desktop.cloud.sign_out_for_code(Some("Billing".into())))
    .expect("signed out");
  assert!(cloud.signed_out(), "the cloud ended the session");
  let SignIn::SignedOut { ended: Some(ended), .. } = desktop.cloud.sign_in_state() else {
    panic!("signed out with a reason");
  };
  assert_eq!(ended.reason, CloudSignInEnd::TwoFactor);
  assert_eq!(ended.invite.as_deref(), Some("Billing"));
  assert_eq!(ended.email, EMAIL);
}

#[test]
fn two_factor_a_kept_invite_refused_for_a_code_stays_until_a_sign_in_with_one() {
  let cloud = AccountCloud::start();
  let (server, audience) = linked_server(&cloud);
  let invite = server.email_invite(EMAIL);
  let desktop = Desktop::signed_in(&cloud);
  cloud.name_server(&audience, &server.url(), None);
  cloud.invite(&invite, None);
  let listed = desktop.block_on(desktop.cloud.invites()).expect("invites").remove(0);
  // An earlier ask took a token for it, so the cloud lists it no more: it is kept here only.
  let kept = AcceptedInvite {
    asks: 1,
    ..AcceptedInvite::new(
      (listed.id, audience.parse().expect("a server id"), "Kontur team".into()),
      None,
      Timestamp::from_unix_millis(listed.expires_at),
      Timestamp::now(),
    )
  };
  desktop.keep_accepted(&kept);
  cloud.session_without_two_factor();
  let refused = desktop
    .block_on(desktop.cloud.finish_accept(listed.id))
    .expect_err("refused");
  assert!(matches!(refused, CloudError::TwoFactorSession), "{refused:?}");
  let still = desktop.block_on(desktop.cloud.accepted_invites()).expect("kept");
  assert_eq!(still.len(), 1, "the invite is not lost");
  assert_eq!(still[0].next_check_at, None, "and not asked on its own");
  assert!(still[0].needs_code, "until a sign-in with a code");

  cloud.require_codes();
  desktop
    .block_on(desktop.cloud.sign_out_for_code(Some("Kontur team".into())))
    .expect("signed out");
  assert_eq!(
    desktop.block_on(desktop.cloud.accepted_invites()).expect("kept").len(),
    1,
    "kept through the sign-out for a code"
  );
  let challenge = password_step(&desktop);
  finish(&desktop, &challenge, (&cloud.next_code(), CodeKind::App)).expect("signed in with a code");
  let joined = desktop
    .block_on(desktop.cloud.finish_accept(listed.id))
    .expect("joined");
  assert!(matches!(joined.joined, CloudJoined::Joined(_)), "{joined:?}");
}

#[test]
fn two_factor_another_account_forgets_the_invites_kept_for_a_code() {
  let cloud = AccountCloud::start();
  let desktop = Desktop::signed_in(&cloud);
  let kept = AcceptedInvite::new(
    (
      "0191c7c0-0000-7000-8000-00000000abcd".parse().expect("an invite id"),
      "0191c7c0-0000-7000-8000-00000000dcba".parse().expect("a server id"),
      "Kontur team".into(),
    ),
    None,
    Timestamp::from_unix_millis(Timestamp::now().unix_millis() + 86_400_000),
    Timestamp::now(),
  );
  desktop.keep_accepted(&kept);
  desktop
    .block_on(desktop.cloud.sign_out_for_code(None))
    .expect("signed out");
  desktop
    .block_on(
      desktop
        .cloud
        .sign_in(OTHER_EMAIL.into(), Credential::new(PASSWORD.into())),
    )
    .expect("another account");
  assert!(
    desktop
      .block_on(desktop.cloud.accepted_invites())
      .expect("kept")
      .is_empty(),
    "Bohdan's invite is not Olena's"
  );
}

#[test]
fn two_factor_a_challenge_ends_by_this_computers_clock() {
  let cloud = AccountCloud::start();
  cloud.require_codes();
  let desktop = Desktop::new(&cloud.url());
  let challenge = password_step(&desktop);
  let now = Instant::now();
  assert!(!challenge.ended_by(now), "it has just arrived");
  assert!(!challenge.ended_by(now + CHALLENGE_LIFE - Duration::from_secs(5)));
  assert!(
    challenge.ended_by(now + CHALLENGE_LIFE),
    "its five minutes, counted from its arrival"
  );
}
