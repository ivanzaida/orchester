//! The application database's Orchester Cloud rows: the account signed in, the cloud's address, and why the last
//! sign-in ended, also through the two-factor migrations.

use orchester_core::time::Timestamp;
use orchester_store::{AppDb, CloudSignInEnd, EndedCloudSignIn, StoredCloudAccount};

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

#[test]
fn the_cloud_account_is_one_row_and_the_address_a_setting() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let mut db = AppDb::open(&dir.path().join("orchester.db")).expect("open");
  assert_eq!(db.cloud_account().expect("account"), None);
  assert_eq!(db.cloud_address().expect("address"), None);
  let account = StoredCloudAccount {
    account: orchester_core::id::CloudAccountId::generate(),
    email: "ivan@example.com".into(),
    display_name: "Ivan Zaida".into(),
    device_name: "IVAN-PC".into(),
    signed_in_at: at(5),
    cloud_address: "https://cloud.test.lurq.dev".into(),
  };
  db.set_cloud_account(Some(&account)).expect("signed in");
  let other = StoredCloudAccount {
    email: "maria@example.com".into(),
    ..account.clone()
  };
  db.set_cloud_account(Some(&other)).expect("another account replaces it");
  assert_eq!(db.cloud_account().expect("account"), Some(other));
  db.set_cloud_account(None).expect("signed out");
  assert_eq!(db.cloud_account().expect("account"), None);
  db.set_cloud_address(Some("https://cloud.test.lurq.dev"))
    .expect("a test cloud");
  assert_eq!(
    db.cloud_address().expect("address").as_deref(),
    Some("https://cloud.test.lurq.dev")
  );
}

#[test]
fn why_the_last_cloud_sign_in_ended_is_one_row() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let mut db = AppDb::open(&dir.path().join("orchester.db")).expect("open");
  assert_eq!(db.cloud_sign_in_ended().expect("ended"), None);
  let revoked = EndedCloudSignIn {
    email: "ivan@example.com".into(),
    reason: CloudSignInEnd::Revoked,
    ended_at: at(7),
    invite: None,
  };
  db.set_cloud_sign_in_ended(Some(&revoked)).expect("recorded");
  assert_eq!(db.cloud_sign_in_ended().expect("ended"), Some(revoked.clone()));
  let reset = EndedCloudSignIn {
    reason: CloudSignInEnd::PasswordReset,
    ..revoked.clone()
  };
  db.set_cloud_sign_in_ended(Some(&reset))
    .expect("the newer one replaces it");
  assert_eq!(db.cloud_sign_in_ended().expect("ended"), Some(reset));
  let for_a_code = EndedCloudSignIn {
    reason: CloudSignInEnd::TwoFactor,
    invite: Some("Billing".into()),
    ..revoked
  };
  db.set_cloud_sign_in_ended(Some(&for_a_code))
    .expect("signed out to sign in with a code");
  assert_eq!(db.cloud_sign_in_ended().expect("ended"), Some(for_a_code));
  db.set_cloud_sign_in_ended(None).expect("signed in again");
  assert_eq!(db.cloud_sign_in_ended().expect("ended"), None);
}

/// The schema before the two-factor migrations (18: the sign-in end, 19: the kept invites' refusal for want of a code).
const BEFORE_TWO_FACTOR: u32 = 17;

#[test]
fn why_the_last_cloud_sign_in_ended_is_kept_through_the_two_factor_migration() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let path = dir.path().join("orchester.db");
  // Before the two-factor migrations: the sign-in end, then the kept invites' refusal for want of a code.
  let before = BEFORE_TWO_FACTOR;
  let raw = AppDb::create_at_version(&path, before).expect("the schema before two-factor authentication");
  raw
    .execute(
      "INSERT INTO cloud_sign_in_ended (id, email, reason, ended_at) VALUES (1, 'ivan@example.com', 'revoked', 7)",
      [],
    )
    .expect("a row of that schema");
  drop(raw);
  let mut db = AppDb::open(&path).expect("migrate");
  assert_eq!(
    db.cloud_sign_in_ended().expect("kept"),
    Some(EndedCloudSignIn {
      email: "ivan@example.com".into(),
      reason: CloudSignInEnd::Revoked,
      ended_at: at(7),
      invite: None,
    })
  );
  let for_a_code = EndedCloudSignIn {
    email: "ivan@example.com".into(),
    reason: CloudSignInEnd::TwoFactor,
    ended_at: at(8),
    invite: Some("Billing".into()),
  };
  db.set_cloud_sign_in_ended(Some(&for_a_code)).expect("the new reason");
  assert_eq!(db.cloud_sign_in_ended().expect("read"), Some(for_a_code));
}
