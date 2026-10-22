//! Storage messages preserve actionable typed causes without exposing SQLite's detail text.
use orchester_store::StoreError;
use orchester_workspace::messages::{Message, store_error};

#[test]
fn storage_messages_preserve_busy_and_locked_causes_without_raw_details() {
  for code in [
    rusqlite::ffi::SQLITE_BUSY,
    rusqlite::ffi::SQLITE_LOCKED,
    rusqlite::ffi::SQLITE_BUSY_SNAPSHOT,
  ] {
    let error = sqlite(code);
    assert!(error.is_locked());
    assert_eq!(store_error(&error), Message::new("errors.databaseLocked"));
  }
  let StoreError::Sqlite(source) = sqlite(rusqlite::ffi::SQLITE_BUSY) else {
    unreachable!("a SQLite error");
  };
  let gave_up = StoreError::Unavailable {
    waited: std::time::Duration::from_secs(60),
    source,
  };
  assert!(gave_up.is_locked());
  assert_eq!(store_error(&gave_up), Message::new("errors.databaseLocked"));
}

#[test]
fn storage_messages_keep_unknown_sqlite_details_private() {
  let error = sqlite(rusqlite::ffi::SQLITE_CONSTRAINT);
  assert!(!error.is_locked());
  assert_eq!(store_error(&error), Message::new("errors.storage"));
}

fn sqlite(code: i32) -> StoreError {
  StoreError::Sqlite(rusqlite::Error::SqliteFailure(
    rusqlite::ffi::Error::new(code),
    Some("private database detail".into()),
  ))
}
