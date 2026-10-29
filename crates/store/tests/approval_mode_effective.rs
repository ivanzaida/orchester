//! Approval modes in the project database (`docs/design/approval-modes.md`): *Allow Autopilot for agents in this
//! project*, a run's switch to Manual for itself, and the migration from auto mode's settings. Run with
//! `cargo test -p orchester-store --test approval_mode_effective approval_mode_effective`.

mod support;

use std::path::{Path, PathBuf};

use orchester_core::{
  run::{AllowAutopilot, Answerer, EffectiveMode, ManualBecause, ManualSwitch},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use rusqlite::{Connection, params};

use self::support::{copied_at_version, started_run};

/// The migrations after approval modes': shared configuration's, ORC-346's request answers, then when and by whom a
/// run was switched.
const LATER_MIGRATIONS: u32 = 3;

fn database(repo: &Path) -> PathBuf {
  repo.join(".orchester").join("orchester.db")
}

/// The schema version before approval modes, of a project whose schema is `latest`.
const fn before_approval_modes(latest: u32) -> u32 {
  latest - LATER_MIGRATIONS - 1
}

fn schema_version(database: &Path) -> u32 {
  Connection::open(database)
    .expect("open")
    .pragma_query_value(None, "user_version", |row| row.get(0))
    .expect("user_version")
}

#[test]
fn approval_mode_effective_project_does_not_allow_autopilot_until_saved() {
  let (_repo, mut db, run, _) = started_run();
  assert_eq!(db.allow_autopilot().expect("read"), AllowAutopilot::OFF);
  db.save_allow_autopilot(AllowAutopilot::ON).expect("save");
  assert_eq!(db.allow_autopilot().expect("read"), AllowAutopilot::ON);
  // Profiles have no approval mode yet, so each counts as Manual and the run is Manual by its profile.
  assert_eq!(
    db.run_approval_mode(run.id()).expect("mode"),
    EffectiveMode::Manual(ManualBecause::Profile)
  );
  db.save_allow_autopilot(AllowAutopilot::OFF).expect("save");
  assert_eq!(db.allow_autopilot().expect("read"), AllowAutopilot::OFF);
}

#[test]
fn approval_mode_effective_run_switch_to_manual_is_kept_for_good() {
  let (_repo, mut db, run, _) = started_run();
  assert!(!db.run_switched_to_manual(run.id()).expect("read"));
  assert_eq!(db.run_manual_switch(run.id()).expect("read"), None);
  let first = ManualSwitch {
    at: Timestamp::from_unix_millis(1_800_000_000_000),
    by: Answerer::Person,
  };
  db.switch_run_to_manual(run.id(), first).expect("switch");
  assert!(db.run_switched_to_manual(run.id()).expect("read"));
  assert_eq!(db.run_manual_switch(run.id()).expect("read"), Some(first));
  // Switching again changes nothing, not even who switched it; allowing Autopilot does not switch the run back.
  let again = ManualSwitch {
    at: Timestamp::from_unix_millis(1_800_000_060_000),
    by: Answerer::Automation,
  };
  db.switch_run_to_manual(run.id(), again).expect("switch again");
  db.save_allow_autopilot(AllowAutopilot::ON).expect("save");
  assert!(db.run_switched_to_manual(run.id()).expect("read"));
  assert_eq!(db.run_manual_switch(run.id()).expect("read"), Some(first));
}

/// A project of the schema before approval modes with auto mode's settings as given (`new_runs`, *approve
/// everything in each run*) and its run's auto mode `run_auto_mode`, opened, and so migrated: whether it allows
/// Autopilot and whether its run was switched to Manual. A run switched so has no time or author of its switch.
fn migrated((new_runs, everything): (bool, bool), run_auto_mode: bool) -> (bool, bool) {
  let (current, db, run, _) = started_run();
  drop(db);
  let latest = schema_version(&database(current.path()));
  let old = copied_at_version(&database(current.path()), before_approval_modes(latest));
  let raw = Connection::open(database(old.path())).expect("open the old database");
  raw
    .execute(
      "INSERT INTO project_auto_mode (id, new_runs, model) VALUES (1, ?1, 'claude-haiku-4-5-20251001')",
      [new_runs],
    )
    .expect("auto mode");
  if everything {
    raw
      .execute(
        "INSERT INTO project_approve_everything (id, turned_on_by) VALUES (1, 'person')",
        [],
      )
      .expect("approve everything");
  }
  raw
    .execute(
      "UPDATE runs SET auto_mode = ?2 WHERE id = ?1",
      params![run.id().to_string(), run_auto_mode],
    )
    .expect("the run's auto mode");
  drop(raw);
  let db = ProjectDb::open(old.path()).expect("migrate");
  assert_eq!(schema_version(&database(old.path())), latest);
  assert_eq!(db.run_manual_switch(run.id()).expect("read"), None);
  (
    db.allow_autopilot().expect("read").is_on(),
    db.run_switched_to_manual(run.id()).expect("read"),
  )
}

#[test]
fn approval_mode_effective_migration_allows_autopilot_only_where_both_auto_mode_settings_were_on() {
  assert_eq!(migrated((true, true), true), (true, false));
  // There a run whose auto mode a person had turned off stays out of Autopilot.
  assert_eq!(migrated((true, true), false), (true, true));
  for (settings, run_auto_mode) in [
    ((true, false), true),
    ((true, false), false),
    ((false, true), false),
    ((false, false), false),
  ] {
    assert_eq!(
      migrated(settings, run_auto_mode),
      (false, false),
      "{settings:?} · run auto mode {run_auto_mode}"
    );
  }
}

#[test]
fn approval_mode_effective_migration_keeps_the_reviewer_and_drops_the_old_settings() {
  let (current, db, ..) = started_run();
  drop(db);
  let latest = schema_version(&database(current.path()));
  let old = copied_at_version(&database(current.path()), before_approval_modes(latest));
  let raw = Connection::open(database(old.path())).expect("open the old database");
  raw
    .execute(
      "INSERT INTO project_auto_mode (id, new_runs, model, connection, effort) \
       VALUES (1, 1, 'claude-sonnet-5-5', '01a1874b-683d-724d-9ce0-b0b04245e644', 'high')",
      [],
    )
    .expect("auto mode");
  drop(raw);
  let db = ProjectDb::open(old.path()).expect("migrate");
  let reviewer = db
    .auto_mode_settings()
    .expect("read")
    .reviewer
    .expect("the reviewer kept");
  assert_eq!(reviewer.model.as_str(), "claude-sonnet-5-5");
  assert_eq!(reviewer.effort.as_deref(), Some("high"));
  drop(db);
  let raw = Connection::open(database(old.path())).expect("open");
  let column = |table: &str, column: &str| -> bool {
    raw
      .query_row(
        &format!("SELECT EXISTS(SELECT 1 FROM pragma_table_info('{table}') WHERE name = ?1)"),
        [column],
        |row| row.get(0),
      )
      .expect("columns")
  };
  assert!(!column("project_auto_mode", "new_runs"));
  assert!(!column("runs", "auto_mode"));
  assert!(column("runs", "manual"));
  let everything: bool = raw
    .query_row(
      "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name = 'project_approve_everything')",
      [],
      |row| row.get(0),
    )
    .expect("tables");
  assert!(!everything);
}
