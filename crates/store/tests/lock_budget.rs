//! A write whose database stays locked longer than a write may wait for it (ORC-74) gives up with an error that says
//! so, how long it waited and the SQLite code, and changes nothing. The time a write waits is shortened for the test,
//! process-wide, so the test has a binary of its own. Run with `cargo test -p orchester-store --test lock_budget
//! lock_budget`.

mod support;

use std::time::{Duration, Instant};

use orchester_core::{
  code_index::CodeIndexSetting,
  connection::stream::AgentEvent,
  store::{ActivityStore, SettingsStore},
  time::Timestamp,
};
use orchester_store::StoreError;

use self::support::{model, started_run};

const BUDGET: Duration = Duration::from_secs(1);

#[test]
fn lock_budget_a_write_gives_up_once_the_lock_stays_held_past_the_budget() {
  orchester_store::set_lock_budget_for_test(BUDGET);
  let (repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), Timestamp::now())
    .expect("invocation");
  let holder = rusqlite::Connection::open(repo.path().join(".orchester").join("orchester.db")).expect("open");
  holder.execute_batch("BEGIN IMMEDIATE").expect("hold the write lock");

  let started = Instant::now();
  let error = db
    .append_activity(&invocation, &[(Timestamp::now(), AgentEvent::TurnStarted)])
    .expect_err("the lock stays held");
  assert!(started.elapsed() >= BUDGET, "gave up after {:?}", started.elapsed());
  assert!(started.elapsed() < BUDGET * 5, "gave up after {:?}", started.elapsed());
  assert!(matches!(error, StoreError::Unavailable { .. }), "{error:?}");
  assert!(error.is_locked());
  let message = error.to_string();
  assert!(
    message.starts_with("the database stayed locked for 1.") && message.ends_with("(SQLITE_BUSY)"),
    "{message}"
  );

  let error = db
    .save_code_index(CodeIndexSetting::Off)
    .expect_err("the lock stays held for every write");
  assert!(error.is_locked());
  assert_eq!(error.to_string(), "database error: database is locked (SQLITE_BUSY)");

  holder.execute_batch("ROLLBACK").expect("let go of the lock");
  assert!(
    db.activity(run.id(), 0).expect("transcript").is_empty(),
    "nothing was written"
  );
}
