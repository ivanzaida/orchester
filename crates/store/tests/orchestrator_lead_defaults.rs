//! OA-10: the ceilings a new subscription that leads to the end takes, kept per project (Project settings ›
//! Orchestrator › *Leading to the end*): a project that never set them, and one from before the setting, has the
//! built-in ones; what is set is kept, a budget included.
//! `cargo test -p orchester-store --test orchestrator_lead_defaults lead_defaults`.

mod support;

use orchester_core::{orchestrator::Ceilings, tokens::TokenBudget};
use orchester_store::ProjectDb;
use support::{copied_at_version, started_run};

#[test]
fn lead_defaults_start_as_the_built_in_ones_and_keep_what_is_set() {
  let (repo, mut db, _run, _execution) = started_run();
  assert_eq!(db.lead_defaults().expect("read"), Ceilings::default());

  let budget = TokenBudget::new(40_000_000).expect("a budget");
  let set = Ceilings::new((5, 10), (3, 100), Some(budget)).expect("ceilings");
  db.save_lead_defaults(&set).expect("kept");
  assert_eq!(db.lead_defaults().expect("read"), set);
  drop(db);
  let reopened = ProjectDb::open(repo.path()).expect("open");
  assert_eq!(reopened.lead_defaults().expect("read"), set, "kept with the project");

  let mut reopened = reopened;
  let none = Ceilings::new((1, 0), (0, 1), None).expect("the least");
  reopened.save_lead_defaults(&none).expect("kept");
  assert_eq!(
    reopened.lead_defaults().expect("read"),
    none,
    "no budget is kept as none"
  );
}

#[test]
fn lead_defaults_of_a_project_from_before_the_setting_are_the_built_in_ones() {
  let (repo, db, _run, _execution) = started_run();
  drop(db);
  let current = repo.path().join(".orchester/orchester.db");
  let latest = {
    let connection = rusqlite::Connection::open(&current).expect("open");
    connection
      .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
      .expect("version")
  };
  let before = (1..latest)
    .rev()
    .find(|&version| {
      let old_repo = tempfile::tempdir().expect("temporary directory");
      let old = ProjectDb::create_at_version(old_repo.path(), version).expect("an old database");
      !old
        .prepare("SELECT 1 FROM pragma_table_info('orchestrator_watch') WHERE name = 'lead_ceilings'")
        .expect("schema")
        .exists([])
        .expect("schema")
    })
    .expect("a schema before the leading defaults");
  let old = copied_at_version(&current, before);
  let upgraded = ProjectDb::open(old.path()).expect("migrate");
  assert_eq!(upgraded.lead_defaults().expect("read"), Ceilings::default());
}
