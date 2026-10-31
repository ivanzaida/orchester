//! Settings › Updates in the application database (ORC-413).
//!
//! `cargo test -p orchester-store --test app_updates app_updates`

use orchester_core::{
  app_update::{Channel, UpdateSettings},
  time::Timestamp,
};
use orchester_store::AppDb;

/// The schema before the update settings (30).
const BEFORE_UPDATES: u32 = 29;

#[test]
fn app_updates_settings_and_the_last_check_are_kept() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let path = dir.path().join("orchester.db");
  let mut db = AppDb::open(&path).expect("create");
  assert_eq!(db.update_settings().expect("read"), UpdateSettings::default());
  assert_eq!(db.update_checked_at().expect("read"), None);

  db.set_update_channel(Channel::Beta).expect("channel");
  db.set_update_automatically(false).expect("automatic");
  db.set_update_checked_at(Timestamp::from_unix_millis(1_790_000_000_000))
    .expect("checked");
  drop(db);

  let db = AppDb::open(&path).expect("reopen");
  assert_eq!(
    db.update_settings().expect("read"),
    UpdateSettings {
      channel: Channel::Beta,
      automatic: false
    }
  );
  assert_eq!(
    db.update_checked_at().expect("read"),
    Some(Timestamp::from_unix_millis(1_790_000_000_000))
  );
}

#[test]
fn app_updates_are_stable_and_automatic_in_a_database_that_never_stored_them() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let path = dir.path().join("orchester.db");
  let raw = AppDb::create_at_version(&path, BEFORE_UPDATES).expect("the schema before updates");
  raw
    .execute("UPDATE app_settings SET sends_error_reports = 0", [])
    .expect("a setting of that schema");
  drop(raw);

  let db = AppDb::open(&path).expect("migrate");
  assert!(!db.sends_error_reports().expect("kept"));
  assert_eq!(
    db.update_settings().expect("read"),
    UpdateSettings {
      channel: Channel::Stable,
      automatic: true
    }
  );
}
