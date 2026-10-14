//! Outputs of step executions and the migration that made step budgets tokens.

mod support;

use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, StepKey, VersionNumber},
  run::{StepOutcome, StepOutput},
  store::{ActivityStore, ResourceStore, RunStore},
};
use orchester_store::ProjectDb;
use rusqlite::Connection;

use self::support::{at, started_run};

fn output(run: &orchester_core::run::Run, text: &str, outcome: Option<StepOutcome>, millis: i64) -> StepOutput {
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  StepOutput {
    run: run.id(),
    execution,
    step: StepKey::new("implement").expect("key"),
    outcome,
    text: text.to_owned(),
    unverified: Vec::new(),
    created_at: at(millis),
  }
}

#[test]
fn step_outputs_round_trip_once_per_execution() {
  let (_repo, mut db, run, _) = started_run();
  assert!(db.step_outputs(run.id()).expect("outputs").is_empty());
  let first = output(&run, "Added the sign-in flow.", Some(StepOutcome::Done), 10);
  db.save_step_output(&first).expect("save");
  // Saving again keeps the first output: an execution has one.
  db.save_step_output(&output(&run, "Something else.", None, 11))
    .expect("save again");
  assert_eq!(db.step_outputs(run.id()).expect("outputs"), [first]);
}

/// A project database at schema 9, before step budgets were tokens (see the file).
const SCHEMA_9: &str = include_str!("fixtures/schema_9_money_step_budgets.sql");
const PIPELINE: &str = "01920000-0000-7000-8000-000000000007";
const PIPELINE_VERSION: &str = "01920000-0000-7000-8000-000000000008";
const RUN: &str = "01920000-0000-7000-8000-000000000009";

#[test]
fn step_outputs_migration_reads_money_step_budgets_as_none_and_keeps_them() {
  let repo = tempfile::tempdir().expect("temporary directory");
  ProjectDb::create_at_version(repo.path(), 9)
    .expect("schema 9")
    .execute_batch(SCHEMA_9)
    .expect("rows of schema 9");

  let db = ProjectDb::open(repo.path()).expect("migrate");
  let pipeline: ResourceId = PIPELINE.parse().expect("fixture id");
  let version = db
    .version::<Pipeline>(pipeline, VersionNumber::FIRST)
    .expect("read")
    .expect("published version");
  assert_eq!(version.body().steps()[0].budget, None);
  let draft = db.draft::<Pipeline>(pipeline).expect("read").expect("draft");
  assert_eq!(draft.body().steps()[0].budget, None);
  // Runs pinned the version before; their snapshots still read.
  assert!(db.run(RUN.parse().expect("fixture id")).expect("read").is_some());
  drop(db);

  let connection = Connection::open(repo.path().join(".orchester").join("orchester.db")).expect("open");
  let mut kept = connection
    .prepare("SELECT owner, owner_id, step_key, micros, currency FROM money_step_budgets_before_tokens ORDER BY owner")
    .expect("prepare");
  let kept: Vec<(String, String, String, i64, String)> = kept
    .query_map([], |row| {
      Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
    })
    .expect("query")
    .collect::<Result<_, _>>()
    .expect("rows");
  assert_eq!(
    kept,
    [
      (
        "draft".to_owned(),
        PIPELINE.to_owned(),
        "implement".to_owned(),
        1_000_000,
        "EUR".to_owned()
      ),
      (
        "version".to_owned(),
        PIPELINE_VERSION.to_owned(),
        "implement".to_owned(),
        2_500_000,
        "USD".to_owned()
      ),
    ]
  );
}
