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

use account_cloud::{AccountCloud, EMAIL, PASSWORD};
use desktop::Desktop;
use orchester_api::Credential;
use orchester_core::{secret::SecretVault, time::Timestamp};
use orchester_store::{CloudSignInEnd, EndedCloudSignIn};
use orchester_workspace::{
  secrets::Vault,
  sharing::{
    Servers,
    cloud::{Cloud, CloudError, Reach, SignIn, keep_checking},
  },
};

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
  assert_eq!(desktop.cloud.sign_in_state(), SignIn::SignedIn { email: EMAIL.into() });
  assert_eq!(desktop.cloud.reach(), Reach::Reached);
  assert!(restarted(&desktop).is_signed_in(), "the sign-in holds across starts");

  desktop.block_on(desktop.cloud.sign_out()).expect("sign out");
  let Some(Some(signed_out)) = ended(&desktop) else {
    panic!("signed out with a reason: {:?}", desktop.cloud.sign_in_state());
  };
  assert_eq!((signed_out.email.as_str(), signed_out.reason), (EMAIL, CloudSignInEnd::SignedOut));
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
  assert!(desktop.cloud.is_signed_in(), "offline-first: the stored sign-in keeps the app open");
  let Reach::Unreachable { since } = desktop.cloud.reach() else {
    panic!("unreachable: {:?}", desktop.cloud.reach());
  };
  assert!(since >= before);
  let again = desktop.block_on(desktop.cloud.check_sign_in());
  assert!(again.is_err());
  assert_eq!(desktop.cloud.reach(), Reach::Unreachable { since }, "since the first failure");

  cloud.set_down(false);
  desktop.block_on(desktop.cloud.check_sign_in()).expect("checked");
  assert_eq!(desktop.cloud.reach(), Reach::Reached);
  assert!(desktop.cloud.is_signed_in());
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
    assert!(desktop.vault.read(&desktop.device_key()).is_err(), "the token left the keychain");
    assert!(!restarted(&desktop).is_signed_in(), "{reason:?}: the next start shows the gate");
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
  assert_eq!(ended(&desktop).flatten().map(|ended| ended.reason), Some(CloudSignInEnd::Revoked));

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
