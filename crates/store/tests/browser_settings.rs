//! Settings › Browser's program in the application database (`docs/design/browser.md`, "Finding the browser"): none
//! chosen until a person chooses one, the choice kept, and the read-only opening the runner reads it with. The Browser
//! window's switch is tested in `app_db.rs`. Run with `cargo test -p orchester-store --test browser_settings`.

use std::path::Path;

use orchester_store::{AppDb, StoreError, location::browser_profiles_beside};

/// The schema before Settings › Browser's program (31).
const BEFORE_BROWSER: u32 = 30;

#[test]
fn browser_program_is_the_one_found_until_chosen_and_the_choice_is_kept() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let path = dir.path().join("orchester.db");
  let mut db = AppDb::open(&path).expect("create");
  assert_eq!(
    db.browser_program().expect("read"),
    None,
    "none chosen in a new database"
  );
  let chosen = dir.path().join("Chromium.app");
  db.set_browser_program(Some(&chosen)).expect("choose");
  drop(db);

  let mut db = AppDb::open(&path).expect("reopen");
  assert_eq!(db.browser_program().expect("read"), Some(chosen));
  db.set_browser_program(None).expect("back to the one found");
  assert_eq!(db.browser_program().expect("read"), None);
}

#[test]
fn browser_program_of_a_database_that_never_stored_it_is_none() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let path = dir.path().join("orchester.db");
  drop(AppDb::create_at_version(&path, BEFORE_BROWSER).expect("the schema before Settings › Browser"));

  let db = AppDb::open(&path).expect("migrate");
  assert_eq!(db.browser_program().expect("read"), None);
  assert!(db.browser_window_opens_itself().expect("read"), "on until turned off");
}

#[test]
fn browser_settings_read_only_are_read_and_never_written() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let path = dir.path().join("orchester.db");
  let chosen = Path::new("/Applications/Brave Browser.app");
  AppDb::open(&path)
    .expect("create")
    .set_browser_program(Some(chosen))
    .expect("choose");

  let mut read = AppDb::open_read_only(&path).expect("open to read");
  assert_eq!(read.browser_program().expect("read"), Some(chosen.to_path_buf()));
  assert!(
    matches!(read.set_browser_program(None), Err(StoreError::Sqlite(_))),
    "a read-only database refuses writes"
  );

  let older = dir.path().join("older.db");
  drop(AppDb::create_at_version(&older, BEFORE_BROWSER).expect("an older schema"));
  assert!(
    matches!(AppDb::open_read_only(&older), Err(StoreError::SchemaTooOld { .. })),
    "only the app that owns it migrates it"
  );
}

#[test]
fn browser_profiles_are_beside_the_application_database() {
  assert_eq!(
    browser_profiles_beside(Path::new("/data/orchester.db")),
    Some(Path::new("/data/browser").to_path_buf())
  );
}
