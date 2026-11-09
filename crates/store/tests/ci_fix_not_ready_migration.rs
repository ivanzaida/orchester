//! Migration 159, a Fix CI that waits because its pipeline is not ready here: a database before it, with Fix CI events
//! sent by hand and automatically and one that stopped, opens with every row kept, takes a `not_ready` event, and still
//! refuses to change or delete one (the events are append-only).
//! `cargo test -p orchester-store --test ci_fix_not_ready_migration ci_fix_not_ready_migration`.

mod support;

use orchester_core::{
  id::MessageId,
  pull_request::ci::fix::{FixEvent, FixEventKind, FixLimit, FixedBy},
};
use orchester_store::ProjectDb;
use rusqlite::Connection;
use support::{at, copied_at_version, started_run};

/// The schema before `run_ci_fix_events` took the kind `not_ready`. Migrations are append-only, so a later one still
/// applies on open.
const BEFORE_NOT_READY: u32 = 158;

fn event(kind: FixEventKind, millis: i64) -> FixEvent {
  FixEvent {
    at: at(millis),
    number: 7,
    failure: "checks:build".to_owned(),
    failing: vec!["build".to_owned(), "lint".to_owned()],
    kind,
  }
}

#[test]
fn ci_fix_not_ready_migration_keeps_every_event_and_takes_the_new_kind_still_append_only() {
  let (repo, mut db, run, _execution) = started_run();
  let hand = FixEventKind::Sent {
    message: MessageId::generate(),
    by: FixedBy::Hand,
  };
  let automatic = FixEventKind::Sent {
    message: MessageId::generate(),
    by: FixedBy::Automatically {
      count: 2,
      limit: FixLimit::Three,
    },
  };
  let stopped = FixEventKind::Stopped { limit: FixLimit::Two };
  let before: Vec<FixEvent> = [hand, automatic, stopped]
    .into_iter()
    .zip([10, 20, 30])
    .map(|(kind, millis)| event(kind, millis))
    .collect();
  for recorded in &before {
    db.record_ci_fix(run.id(), recorded).expect("an event of the old kinds");
  }
  drop(db);

  let current = repo.path().join(".orchester").join("orchester.db");
  let old = copied_at_version(&current, BEFORE_NOT_READY);
  let old_file = old.path().join(".orchester").join("orchester.db");
  let waiting = event(
    FixEventKind::NotReady {
      message: MessageId::generate(),
    },
    40,
  );
  // The constant names the schema before the migration: it refuses the new kind.
  {
    let old_db = Connection::open(&old_file).expect("the old database");
    let refused = old_db.execute(
      "INSERT INTO run_ci_fix_events (run_id, at, pull_request, failure, failing, kind, message_id) \
       VALUES (?1, 40, 7, 'checks:build', '[]', 'not_ready', ?2)",
      [run.id().to_string(), MessageId::generate().to_string()],
    );
    assert!(refused.is_err(), "before the migration `not_ready` is no kind");
    old_db.close().expect("closed");
  }

  let mut migrated = ProjectDb::open(old.path()).expect("migrate the existing project");
  assert_eq!(
    migrated.ci_fixes(run.id()).expect("read").events,
    before,
    "every event is kept, as it was"
  );
  migrated
    .record_ci_fix(run.id(), &waiting)
    .expect("the migrated table takes a not_ready event");
  let mut after = before;
  after.push(waiting);
  assert_eq!(migrated.ci_fixes(run.id()).expect("read").events, after);
  drop(migrated);

  let rebuilt = Connection::open(&old_file).expect("the migrated database");
  let update = rebuilt.execute("UPDATE run_ci_fix_events SET failure = 'changed'", []);
  assert!(update.is_err_and(|error| error.to_string().contains("append-only")));
  let delete = rebuilt.execute("DELETE FROM run_ci_fix_events", []);
  assert!(delete.is_err_and(|error| error.to_string().contains("append-only")));
}
