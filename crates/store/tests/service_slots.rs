//! *Services at once* (ORC-248): the computer's limit in the application database and each project's share of it with
//! the project's limits, both kept within 1 to 32 and through their migrations.

mod support;

use orchester_core::{
  limits::{ComputerServiceSlots, ProjectLimits, QaSlots, ServiceSlots},
  store::SettingsStore,
};
use orchester_store::{AppDb, ProjectDb};
use rusqlite::Connection;

use self::support::started_run;

#[test]
fn service_slots_the_computers_limit_is_8_until_changed_and_kept() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let path = dir.path().join("orchester.db");
  let mut db = AppDb::open(&path).expect("open");
  assert_eq!(db.service_slots().expect("read"), ComputerServiceSlots::DEFAULT);
  assert_eq!(ComputerServiceSlots::DEFAULT.get().get(), 8);
  db.set_service_slots(ComputerServiceSlots::new(12).expect("in range"))
    .expect("save");
  drop(db);
  let read = AppDb::open_read_only(&path).expect("read as the runner does");
  assert_eq!(read.service_slots().expect("read").get().get(), 12);
  let out_of_range = Connection::open(&path)
    .expect("open")
    .execute("UPDATE app_settings SET service_slots = 0", []);
  assert!(out_of_range.is_err(), "the schema keeps the limit within 1 to 32");
}

#[test]
fn service_slots_a_projects_share_persists_with_its_other_limits() {
  let (repo, mut db, ..) = started_run();
  assert_eq!(
    db.project_limits().expect("limits").service_slots(),
    ServiceSlots::DEFAULT
  );
  assert_eq!(ServiceSlots::DEFAULT.get().get(), 4);
  let share = ServiceSlots::new(6).expect("in range");
  let changed = ProjectLimits::DEFAULT
    .with_qa_slots(QaSlots::new(3).expect("in range"))
    .with_service_slots(share);
  db.save_project_limits(&changed).expect("save");
  let read = ProjectDb::open(repo.path())
    .expect("open")
    .project_limits()
    .expect("limits");
  assert_eq!(read.service_slots(), share);
  assert_eq!(read, changed);
  let out_of_range = Connection::open(repo.path().join(".orchester").join("orchester.db"))
    .expect("open")
    .execute("UPDATE project_limits SET service_slots = 33", []);
  assert!(out_of_range.is_err(), "the schema keeps a share within 1 to 32");
}

#[test]
fn service_slots_a_share_counts_as_the_computers_limit_when_it_is_larger() {
  let share = ServiceSlots::new(6).expect("in range");
  let limit = ComputerServiceSlots::new(4).expect("in range");
  assert!(!share.fits(limit));
  assert_eq!(share.within(limit).get(), 4);
  assert_eq!(share.within(ComputerServiceSlots::DEFAULT).get(), 6);
  assert!(ServiceSlots::new(0).is_err() && ServiceSlots::new(33).is_err());
}
