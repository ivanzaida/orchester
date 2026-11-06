//! What the store keeps of merge attempts: every attempt of a run in order, never changed or removed, with who asked,
//! from where, its outcome, cause and stages; the attempts left open; the runs whose last attempt did not land; and the
//! merged runs, newest first, a page at a time.

mod support;

use std::num::NonZeroU32;

use orchester_core::{
  id::RunId,
  integration::{
    MergeKind, RunMerge,
    attempt::{
      AttemptOutcome, AttemptStage, CauseKind, MergeAsker, MergeAttempt, MergeVia, OpenAttempt, OsCause, StageResult,
    },
  },
  revision::Revision,
  run::BranchName,
  store::MergeStore,
};
use orchester_store::{ProjectDb, StoreError};
use rusqlite::Connection;

use self::support::{at, started_run};

fn revision(letter: char) -> Revision {
  Revision::new(&letter.to_string().repeat(40)).expect("revision")
}

fn attempt(run: RunId, at_millis: i64, asked: (MergeAsker, MergeVia), outcome: AttemptOutcome) -> MergeAttempt {
  MergeAttempt::new((run, at(at_millis)), asked, outcome, Some("held by another operation"))
}

const PERSON: (MergeAsker, MergeVia) = (MergeAsker::Person, MergeVia::TaskPage);

#[test]
fn merge_attempts_are_recorded_in_order_and_read_newest_first() {
  let (repo, mut db, run, _) = started_run();
  let refused = attempt(
    run.id(),
    10,
    PERSON,
    AttemptOutcome::Refused {
      stage: AttemptStage::IntegrationLock,
    },
  );
  let retry = (
    MergeAsker::Retry {
      number: NonZeroU32::new(2).expect("two"),
    },
    MergeVia::MergeQueue,
  );
  let merged = MergeAttempt::new(
    (run.id(), at(20)),
    retry,
    AttemptOutcome::Merged {
      revision: revision('c'),
      kind: MergeKind::Squash,
    },
    None,
  );
  assert_eq!(db.record_merge_attempt(&refused).expect("recorded"), 1);
  assert_eq!(db.record_merge_attempt(&merged).expect("recorded"), 2);
  let read = ProjectDb::open(repo.path())
    .expect("open")
    .merge_attempts(run.id())
    .expect("read");
  assert_eq!(read.len(), 2);
  assert_eq!((read[0].sequence, &read[0].attempt), (2, &merged));
  assert_eq!((read[1].sequence, &read[1].attempt), (1, &refused));
  assert_eq!(read[0].task, run.task());
  assert_eq!(
    read[1].attempt.stages()[..3],
    [
      (AttemptStage::ProjectDatabase, StageResult::Passed),
      (AttemptStage::IntegrationLock, StageResult::Refused),
      (AttemptStage::ScratchFolder, StageResult::NotReached),
    ],
    "listed as they run: the database is held first, then the lease that refused"
  );
  assert!(
    read[0]
      .attempt
      .stages()
      .iter()
      .all(|(_, result)| *result == StageResult::Passed)
  );
}

#[test]
fn merge_attempts_keep_the_kind_of_their_cause() {
  let (repo, mut db, run, _) = started_run();
  let in_use = CauseKind::Os {
    what: OsCause::UsedByAnotherProgram,
    code: Some(32),
  };
  let refused = attempt(
    run.id(),
    10,
    PERSON,
    AttemptOutcome::Refused {
      stage: AttemptStage::ProjectDatabase,
    },
  )
  .with_kind(Some(in_use));
  let silent = attempt(
    run.id(),
    20,
    PERSON,
    AttemptOutcome::GitStopped {
      stage: AttemptStage::Merge,
    },
  )
  .with_kind(Some(CauseKind::Unanswered { seconds: 120 }));
  let unknown = attempt(
    run.id(),
    30,
    PERSON,
    AttemptOutcome::Refused {
      stage: AttemptStage::Policy,
    },
  );
  for recorded in [&refused, &silent, &unknown] {
    db.record_merge_attempt(recorded).expect("recorded");
  }
  let read = ProjectDb::open(repo.path())
    .expect("open")
    .merge_attempts(run.id())
    .expect("read");
  let kinds: Vec<_> = read.iter().map(|recorded| recorded.attempt.kind()).collect();
  assert_eq!(
    kinds,
    [None, Some(CauseKind::Unanswered { seconds: 120 }), Some(in_use)]
  );
}

#[test]
fn merge_attempts_recorded_with_the_lock_listed_first_are_read_in_the_order_the_stages_run() {
  let (repo, mut db, run, _) = started_run();
  let refused = attempt(
    run.id(),
    10,
    PERSON,
    AttemptOutcome::Refused {
      stage: AttemptStage::ProjectDatabase,
    },
  );
  db.record_merge_attempt(&refused).expect("recorded");
  // How an attempt recorded before the stages were listed as they run kept them: the lock first.
  let earlier = r#"{"schema":1,"body":[["integration_lock","not_reached"],["project_database","refused"],
    ["scratch_folder","not_reached"],["checkout_identity","not_reached"],["git_metadata","not_reached"],
    ["existing_result","not_reached"],["merge","not_reached"],["record","not_reached"]]}"#;
  let raw = Connection::open(repo.path().join(".orchester/orchester.db")).expect("open raw database");
  raw
    .execute_batch("DROP TRIGGER merge_attempts_no_update")
    .expect("the record may be rewritten here");
  raw
    .execute("UPDATE merge_attempts SET stages = ?1", [earlier])
    .expect("an earlier record's listing");
  let read = ProjectDb::open(repo.path())
    .expect("open")
    .merge_attempts(run.id())
    .expect("read");
  assert_eq!(
    read[0].attempt.stages()[..2],
    [
      (AttemptStage::ProjectDatabase, StageResult::Refused),
      (AttemptStage::IntegrationLock, StageResult::NotReached),
    ]
  );
}

#[test]
fn merge_attempts_are_append_only() {
  let (repo, mut db, run, _) = started_run();
  db.record_merge_attempt(&attempt(
    run.id(),
    10,
    PERSON,
    AttemptOutcome::Conflicts {
      files: vec!["a.rs".to_owned()],
    },
  ))
  .expect("recorded");
  let raw = Connection::open(repo.path().join(".orchester/orchester.db")).expect("open raw database");
  let tables: i64 = raw
    .query_row(
      "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('merge_attempts', 'merge_attempts_open')",
      [],
      |row| row.get(0),
    )
    .expect("schema");
  assert_eq!(tables, 2, "the migration made both tables");
  let updated = raw.execute("UPDATE merge_attempts SET outcome = 'merged'", []);
  assert!(updated.is_err_and(|error| error.to_string().contains("append-only")));
  let deleted = raw.execute("DELETE FROM merge_attempts", []);
  assert!(deleted.is_err_and(|error| error.to_string().contains("append-only")));
  assert_eq!(db.merge_attempts(run.id()).expect("read").len(), 1);
}

#[test]
fn merge_attempts_of_an_unknown_run_are_refused() {
  let (_repo, mut db, _run, _) = started_run();
  let unknown = RunId::generate();
  let refused = db.record_merge_attempt(&attempt(unknown, 10, PERSON, AttemptOutcome::Moved { stage: None }));
  assert!(matches!(refused, Err(StoreError::RunNotFound(id)) if id == unknown));
}

#[test]
fn merge_attempts_open_one_ends_with_its_record() {
  let (_repo, mut db, run, _) = started_run();
  let open = OpenAttempt {
    run: run.id(),
    at: at(10),
    asked_by: MergeAsker::Automation,
    via: MergeVia::MergeRun,
    stage: AttemptStage::ExistingResult,
  };
  db.open_merge_attempt(&open).expect("open");
  db.open_merge_attempt(&OpenAttempt {
    stage: AttemptStage::Merge,
    ..open.clone()
  })
  .expect("at the next stage");
  let left = db.open_merge_attempts().expect("read");
  assert_eq!(left.len(), 1);
  assert_eq!(left[0].stage, AttemptStage::Merge);
  let interrupted = left[0].interrupted(&[]);
  assert_eq!(
    interrupted.outcome(),
    &AttemptOutcome::Interrupted {
      stage: AttemptStage::Merge
    }
  );
  assert_eq!(interrupted.stages()[0], (AttemptStage::Policy, StageResult::Passed));
  db.record_merge_attempt(&interrupted).expect("recorded");
  assert!(db.open_merge_attempts().expect("read").is_empty());
  db.open_merge_attempt(&open).expect("open again");
  db.close_merge_attempt(run.id()).expect("closed without a record");
  assert!(db.open_merge_attempts().expect("read").is_empty());
  assert_eq!(db.merge_attempts(run.id()).expect("read").len(), 1);
}

#[test]
fn merge_attempts_unlanded_are_the_runs_last_attempts_that_did_not_land() {
  let (_repo, mut db, run, _) = started_run();
  db.record_merge_attempt(&attempt(
    run.id(),
    10,
    PERSON,
    AttemptOutcome::GitStopped {
      stage: AttemptStage::Merge,
    },
  ))
  .expect("recorded");
  db.record_merge_attempt(&attempt(
    run.id(),
    20,
    PERSON,
    AttemptOutcome::LeftChanged {
      files: vec!["a.rs".to_owned()],
    },
  ))
  .expect("recorded");
  let unlanded = db.unlanded_merge_attempts().expect("read");
  assert_eq!(unlanded.len(), 1, "one per run");
  assert_eq!(unlanded[0].sequence, 2);
  db.save_run_merge(&RunMerge {
    run: run.id(),
    into: BranchName::new("main").expect("branch"),
    revision: revision('d'),
    kind: MergeKind::MergeCommit,
    merged_at: at(30),
  })
  .expect("merged since");
  assert!(db.unlanded_merge_attempts().expect("read").is_empty(), "merged since");
}

#[test]
fn merge_attempts_merged_runs_come_newest_first_a_page_at_a_time() {
  let (_repo, mut db, run, _) = started_run();
  db.save_run_merge(&RunMerge {
    run: run.id(),
    into: BranchName::new("main").expect("branch"),
    revision: revision('e'),
    kind: MergeKind::FastForward,
    merged_at: at(30),
  })
  .expect("merged");
  let first = db.merged_runs(None, 1).expect("read");
  assert_eq!(first.len(), 1);
  assert_eq!(first[0].0, run.task());
  assert_eq!(first[0].1.run, run.id());
  assert!(
    db.merged_runs(Some((at(30), run.id())), 1).expect("read").is_empty(),
    "the page after the last merge is empty"
  );
}
