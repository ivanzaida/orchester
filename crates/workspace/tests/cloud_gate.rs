//! The sign-in gate's side of this computer's cloud account (the person's decision, 2026-10-16): signed out until a
//! sign-in, still signed in while the cloud cannot be reached, and signed out with the cloud's reason when it refuses
//! the token, which the next start remembers.
//!
//! `cargo test -p orchester-workspace --test cloud_gate cloud_gate_`

#[path = "sharing_support/account_cloud.rs"]
mod account_cloud;
#[path = "sharing_support/desktop.rs"]
mod desktop;
#[path = "server_support/mod.rs"]
mod server_support;
#[path = "sharing_support/project.rs"]
mod support;

use std::time::Duration;

use account_cloud::{ACCOUNT, AccountCloud, CheckAnswer, EMAIL, OTHER_ACCOUNT, OTHER_EMAIL, PASSWORD};
use desktop::Desktop;
use orchester_api::{Credential, client::ClientError};
use orchester_core::{secret::SecretVault, time::Timestamp};
use orchester_store::{CloudSignInEnd, EndedCloudSignIn};
use orchester_workspace::{
  secrets::Vault,
  sharing::{
    Servers,
    cloud::{
      BUILD_ADDRESS, BUILD_API_ADDRESS, Cloud, CloudError, PUBLIC_ADDRESS, PUBLIC_API_ADDRESS, RETRY_AFTER_CAP,
      RETRY_CAP, RETRY_EVERY, Reach, SignIn, keep_checking, retry_wait,
    },
  },
};
use server_support::TestServer;

/// The sign-in gate's state of `desktop`, without the time it began.
fn ended(desktop: &Desktop) -> Option<Option<EndedCloudSignIn>> {
  match desktop.cloud.sign_in_state() {
    SignIn::SignedIn { .. } => None,
    SignIn::SignedOut { ended, .. } => Some(ended),
  }
}

/// A new start of `desktop`'s session: what the application database says.
fn restarted(desktop: &Desktop) -> Cloud {
  let vault = Vault::with(desktop.vault.clone());
  let servers = Servers::new(desktop.workspace.clone(), vault.clone());
  Cloud::new(desktop.workspace.clone(), vault, servers)
}

#[test]
fn cloud_gate_is_closed_until_a_sign_in_and_after_signing_out_here() {
  let cloud = AccountCloud::start();
  let desktop = Desktop::new(&cloud.url());
  assert_eq!(ended(&desktop), Some(None), "a first start is signed out");
  assert!(!desktop.cloud.is_signed_in());

  desktop
    .block_on(desktop.cloud.sign_in(EMAIL.into(), Credential::new(PASSWORD.into())))
    .expect("sign in");
  assert_eq!(
    desktop.cloud.sign_in_state(),
    SignIn::SignedIn {
      account: ACCOUNT.parse().expect("an account id"),
      email: EMAIL.into()
    }
  );
  assert_eq!(desktop.cloud.reach(), Reach::Reached);
  assert!(restarted(&desktop).is_signed_in(), "the sign-in holds across starts");

  desktop.block_on(desktop.cloud.sign_out()).expect("sign out");
  let Some(Some(signed_out)) = ended(&desktop) else {
    panic!("signed out with a reason: {:?}", desktop.cloud.sign_in_state());
  };
  assert_eq!(
    (signed_out.email.as_str(), signed_out.reason),
    (EMAIL, CloudSignInEnd::SignedOut)
  );
  let SignIn::SignedOut { ended: kept, .. } = restarted(&desktop).sign_in_state() else {
    panic!("the next start is signed out");
  };
  assert_eq!(kept, Some(signed_out), "the next start remembers why");

  desktop
    .block_on(desktop.cloud.sign_in(EMAIL.into(), Credential::new(PASSWORD.into())))
    .expect("sign in again");
  assert!(restarted(&desktop).is_signed_in());
}

#[test]
fn cloud_gate_stays_open_while_the_cloud_cannot_be_reached() {
  let cloud = AccountCloud::start();
  let desktop = Desktop::signed_in(&cloud);
  cloud.set_down(true);
  let before = Timestamp::now();
  let offline = desktop.block_on(desktop.cloud.check_sign_in());
  assert!(matches!(&offline, Err(error) if error.unanswered()), "{offline:?}");
  assert!(
    desktop.cloud.is_signed_in(),
    "offline-first: the stored sign-in keeps the app open"
  );
  let Reach::Unreachable { since } = desktop.cloud.reach() else {
    panic!("unreachable: {:?}", desktop.cloud.reach());
  };
  assert!(since >= before);
  let again = desktop.block_on(desktop.cloud.check_sign_in());
  assert!(again.is_err());
  assert_eq!(
    desktop.cloud.reach(),
    Reach::Unreachable { since },
    "since the first failure"
  );

  cloud.set_down(false);
  desktop.block_on(desktop.cloud.check_sign_in()).expect("checked");
  assert_eq!(desktop.cloud.reach(), Reach::Reached);
  assert!(desktop.cloud.is_signed_in());
}

/// The public cloud's address before it moved (2026-10-25).
const FORMER_PUBLIC: &str = "https://orchester-cloud.lurq.dev";

#[test]
fn cloud_gate_keeps_a_sign_in_at_the_former_public_address_and_moves_it() {
  let cloud = AccountCloud::start();
  let desktop = Desktop::signed_in(&cloud);
  let stayed = restarted(&desktop);
  assert_eq!(
    stayed.address().expect("address").address.web().as_str(),
    cloud.url(),
    "a test cloud stays"
  );
  assert_eq!(
    stayed.account().expect("account").expect("signed in").cloud_address,
    cloud.url()
  );

  desktop.move_account_to(FORMER_PUBLIC);
  desktop.store_cloud_address(&format!("{FORMER_PUBLIC}/"));
  let token = desktop.vault.read(&desktop.device_key()).expect("the token");
  let moved = restarted(&desktop);
  assert!(moved.is_signed_in(), "the sign-in holds across the move");
  let account = moved.account().expect("account").expect("still signed in");
  assert_eq!(account.cloud_address, PUBLIC_ADDRESS);
  assert_eq!(account.cloud_api_address.as_deref(), Some(PUBLIC_API_ADDRESS));
  assert_eq!(account.email, EMAIL);
  let address = moved.address().expect("address");
  assert_eq!(
    address.default,
    BUILD_ADDRESS == PUBLIC_ADDRESS && BUILD_API_ADDRESS == Some(PUBLIC_API_ADDRESS),
    "the public cloud is the build's own again"
  );
  assert_eq!(address.address.web().as_str(), PUBLIC_ADDRESS);
  assert_eq!(address.address.api().map(|api| api.as_str()), Some(PUBLIC_API_ADDRESS));
  assert_eq!(
    desktop
      .vault
      .read(&desktop.device_key())
      .expect("the token stays")
      .expose(),
    token.expose()
  );
}

#[test]
fn cloud_gate_closes_with_each_reason_the_cloud_gives() {
  for (reason, expected) in [
    (Some("revoked"), CloudSignInEnd::Revoked),
    (Some("password_reset"), CloudSignInEnd::PasswordReset),
    (Some("account_deleted"), CloudSignInEnd::AccountDeleted),
    (Some("signed_out"), CloudSignInEnd::SignedOut),
    (Some("a_reason_from_later"), CloudSignInEnd::Other),
    (None, CloudSignInEnd::Other),
  ] {
    let cloud = AccountCloud::start();
    let desktop = Desktop::signed_in(&cloud);
    cloud.end_token(reason, 1_792_147_957_042);
    let refused = desktop.block_on(desktop.cloud.check_sign_in());
    assert!(matches!(refused, Err(CloudError::SignedOut)), "{reason:?}: {refused:?}");
    let Some(Some(ended)) = ended(&desktop) else {
      panic!("{reason:?}: signed out");
    };
    assert_eq!((ended.email.as_str(), ended.reason), (EMAIL, expected), "{reason:?}");
    if reason.is_some() {
      assert_eq!(ended.ended_at, Timestamp::from_unix_millis(1_792_147_957_042));
    }
    assert!(desktop.cloud.account().expect("account").is_none());
    assert!(
      desktop.vault.read(&desktop.device_key()).is_err(),
      "the token left the keychain"
    );
    assert!(
      !restarted(&desktop).is_signed_in(),
      "{reason:?}: the next start shows the gate"
    );
  }
}

#[test]
fn cloud_gate_closes_when_the_token_left_the_keychain() {
  let cloud = AccountCloud::start();
  let desktop = Desktop::signed_in(&cloud);
  desktop.vault.delete(&desktop.device_key()).expect("gone");
  let refused = desktop.block_on(desktop.cloud.check_sign_in());
  assert!(matches!(refused, Err(CloudError::SignedOut)), "{refused:?}");
  let Some(Some(ended)) = ended(&desktop) else {
    panic!("signed out");
  };
  assert_eq!(ended.reason, CloudSignInEnd::Other);
  assert_eq!(cloud.checks(), 0, "nothing was asked without a token");
}

#[test]
fn cloud_gate_check_runs_at_once_and_after_each_sign_in() {
  let cloud = AccountCloud::start();
  let desktop = Desktop::signed_in(&cloud);
  cloud.end_token(Some("revoked"), 1_792_147_957_042);
  let checking = desktop.runtime.spawn(keep_checking(desktop.cloud.clone()));
  let mut changes = desktop.cloud.follow_sign_in();
  desktop.block_on(async {
    while changes.borrow_and_update().is_signed_in() {
      changes.changed().await.expect("the cloud is kept");
    }
  });
  assert_eq!(
    ended(&desktop).flatten().map(|ended| ended.reason),
    Some(CloudSignInEnd::Revoked)
  );

  desktop
    .block_on(desktop.cloud.sign_in(EMAIL.into(), Credential::new(PASSWORD.into())))
    .expect("sign in again");
  let checks = cloud.checks();
  desktop.block_on(async {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    while cloud.checks() == checks {
      assert!(tokio::time::Instant::now() < deadline, "checked after the new sign-in");
      tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
  });
  assert!(desktop.cloud.is_signed_in());
  checking.abort();
}

/// Why `desktop`'s last sign-in ended, while it is signed out with a reason.
fn reason(desktop: &Desktop) -> Option<CloudSignInEnd> {
  ended(desktop).flatten().map(|ended| ended.reason)
}

#[test]
fn cloud_gate_signing_out_locks_when_the_keychain_refuses_to_remove_the_token() {
  let cloud = AccountCloud::start();
  let desktop = Desktop::signed_in(&cloud);
  desktop.vault.refuse(false, true);
  desktop
    .block_on(desktop.cloud.sign_out())
    .expect("signed out all the same");
  assert_eq!(reason(&desktop), Some(CloudSignInEnd::SignedOut));
  assert!(desktop.cloud.account().expect("account").is_none());
  assert!(
    desktop.vault.holds(&desktop.device_key()),
    "the keychain kept the token, without an account"
  );
  assert!(!restarted(&desktop).is_signed_in(), "the next start shows the gate");

  desktop.vault.refuse(false, false);
  desktop
    .block_on(desktop.cloud.sign_in(EMAIL.into(), Credential::new(PASSWORD.into())))
    .expect("the next sign-in replaces the token");
  desktop.block_on(desktop.cloud.check_sign_in()).expect("checked");
}

#[test]
fn cloud_gate_signing_out_locks_when_the_keychain_does_not_give_the_token() {
  let cloud = AccountCloud::start();
  let desktop = Desktop::signed_in(&cloud);
  desktop.vault.refuse(true, true);
  desktop
    .block_on(desktop.cloud.sign_out())
    .expect("signed out all the same");
  assert_eq!(reason(&desktop), Some(CloudSignInEnd::SignedOut), "You signed out");
  assert!(!restarted(&desktop).is_signed_in());
}

#[test]
fn cloud_gate_a_refusal_locks_when_the_keychain_refuses_to_remove_the_token() {
  let cloud = AccountCloud::start();
  let desktop = Desktop::signed_in(&cloud);
  desktop.vault.refuse(false, true);
  cloud.end_token(Some("account_deleted"), 1_792_147_957_042);
  let refused = desktop.block_on(desktop.cloud.check_sign_in());
  assert!(matches!(refused, Err(CloudError::SignedOut)), "{refused:?}");
  assert_eq!(reason(&desktop), Some(CloudSignInEnd::AccountDeleted));
  assert!(desktop.cloud.account().expect("account").is_none());
  assert!(!restarted(&desktop).is_signed_in());
}

#[test]
fn cloud_gate_locks_when_the_keychain_does_not_give_the_token() {
  let cloud = AccountCloud::start();
  let desktop = Desktop::signed_in(&cloud);
  desktop.vault.refuse(true, true);
  let refused = desktop.block_on(desktop.cloud.check_sign_in());
  assert!(matches!(refused, Err(CloudError::SignedOut)), "{refused:?}");
  assert_eq!(reason(&desktop), Some(CloudSignInEnd::Other));
  assert_eq!(cloud.checks(), 0, "nothing was asked without a token");
  assert!(!restarted(&desktop).is_signed_in());
}

#[test]
fn cloud_gate_signing_out_without_a_token_says_you_signed_out() {
  let cloud = AccountCloud::start();
  let desktop = Desktop::signed_in(&cloud);
  desktop.vault.delete(&desktop.device_key()).expect("gone");
  desktop.block_on(desktop.cloud.sign_out()).expect("signed out");
  assert_eq!(reason(&desktop), Some(CloudSignInEnd::SignedOut));
}

#[test]
fn cloud_gate_a_late_refusal_ends_only_the_token_it_refused() {
  let cloud = AccountCloud::start();
  let desktop = Desktop::signed_in(&cloud);
  cloud.hold_checks();
  let late = desktop.runtime.spawn({
    let cloud = desktop.cloud.clone();
    async move { cloud.check_sign_in().await }
  });
  while cloud.checks_arrived() == 0 {
    std::thread::sleep(Duration::from_millis(10));
  }
  desktop.block_on(desktop.cloud.sign_out()).expect("signed out");
  cloud.end_token(Some("signed_out"), 1_792_147_957_042);
  desktop
    .block_on(
      desktop
        .cloud
        .sign_in(OTHER_EMAIL.into(), Credential::new(PASSWORD.into())),
    )
    .expect("signed in as another account");
  cloud.let_checks_go();
  let late = desktop.block_on(late).expect("the check ran");
  assert!(
    matches!(late, Err(CloudError::SignedOut)),
    "the old token was refused: {late:?}"
  );
  assert_eq!(
    desktop.cloud.sign_in_state(),
    SignIn::SignedIn {
      account: OTHER_ACCOUNT.parse().expect("an account id"),
      email: OTHER_EMAIL.into()
    },
    "the newer sign-in stays"
  );
  assert!(desktop.vault.read(&desktop.device_key()).is_ok(), "its token stays");
  assert!(restarted(&desktop).is_signed_in());
}

#[test]
fn cloud_gate_an_answer_that_is_not_the_clouds_keeps_the_sign_in() {
  for answer in [CheckAnswer::Redirect, CheckAnswer::SignInPage] {
    let cloud = AccountCloud::start();
    let desktop = Desktop::signed_in(&cloud);
    cloud.answer_checks(Some(answer));
    let checked = desktop.block_on(desktop.cloud.check_sign_in());
    assert!(
      matches!(&checked, Err(error) if error.unanswered()),
      "{answer:?}: {checked:?}"
    );
    assert!(desktop.cloud.is_signed_in(), "{answer:?}: still signed in");
    assert!(matches!(desktop.cloud.reach(), Reach::Unreachable { .. }), "{answer:?}");
    assert!(restarted(&desktop).is_signed_in(), "{answer:?}");
  }
}

#[test]
fn cloud_gate_an_untrusted_certificate_keeps_the_sign_in() {
  let cloud = AccountCloud::start();
  let desktop = Desktop::signed_in(&cloud);
  let mut stranger = TestServer::start();
  stranger.serve_self_signed();
  desktop.move_account_to(&stranger.url());
  let checked = desktop.block_on(desktop.cloud.check_sign_in());
  assert!(
    matches!(&checked, Err(CloudError::Client(ClientError::UntrustedCertificate))),
    "{checked:?}"
  );
  assert!(desktop.cloud.is_signed_in());
  assert!(matches!(desktop.cloud.reach(), Reach::Unreachable { .. }));
}

#[test]
fn cloud_gate_waits_as_long_as_a_busy_cloud_asks() {
  for (answer, seconds) in [(CheckAnswer::Busy(120), 120), (CheckAnswer::Limited(240), 240)] {
    let cloud = AccountCloud::start();
    let desktop = Desktop::signed_in(&cloud);
    cloud.answer_checks(Some(answer));
    let error = desktop.block_on(desktop.cloud.check_sign_in()).expect_err("refused");
    assert!(desktop.cloud.is_signed_in(), "{answer:?}");
    assert_eq!(desktop.cloud.reach(), Reach::Reached, "{answer:?}: the cloud answered");
    for spread in [0.0, 1.0] {
      assert_eq!(
        retry_wait(&error, 1, spread),
        Some(Duration::from_secs(seconds)),
        "{answer:?}"
      );
    }
  }
  let cloud = AccountCloud::start();
  let desktop = Desktop::signed_in(&cloud);
  cloud.answer_checks(Some(CheckAnswer::Busy(10 * 24 * 60 * 60)));
  let error = desktop.block_on(desktop.cloud.check_sign_in()).expect_err("refused");
  assert_eq!(
    retry_wait(&error, 1, 0.0),
    Some(RETRY_AFTER_CAP),
    "a wrong Retry-After is capped"
  );
}

#[test]
fn cloud_gate_tries_again_ever_later_while_the_cloud_does_not_answer() {
  let cloud = AccountCloud::start();
  let desktop = Desktop::signed_in(&cloud);
  cloud.set_down(true);
  let error = desktop
    .block_on(desktop.cloud.check_sign_in())
    .expect_err("unreachable");
  let waits: Vec<_> = (1..=8).map(|failures| retry_wait(&error, failures, 0.0)).collect();
  assert_eq!(waits[0], Some(RETRY_EVERY));
  assert_eq!(waits[1], Some(RETRY_EVERY * 2));
  assert_eq!(waits[2], Some(RETRY_EVERY * 4));
  assert_eq!(waits[7], Some(RETRY_CAP), "capped");
  assert_eq!(retry_wait(&error, u32::MAX, 0.0), Some(RETRY_CAP));
  assert_eq!(
    retry_wait(&error, 3, 1.0),
    Some((RETRY_EVERY * 4).mul_f64(0.8)),
    "spread up to a fifth sooner"
  );
  cloud.set_down(false);
  desktop.block_on(desktop.cloud.sign_out()).expect("signed out");
  let error = desktop.block_on(desktop.cloud.check_sign_in()).expect_err("signed out");
  assert_eq!(retry_wait(&error, 1, 0.0), None, "nothing to try again soon");
}
