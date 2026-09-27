//! Outputs of step executions and the migration that made step budgets tokens.

mod support;

use orchester_core::{
  resource::{Pipeline, StepKey},
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

/// Takes a migrated project database back to its shape before step outputs and token step budgets
/// (schema 9), with the published pipeline's step budget and a draft's written as money, as they were
/// stored then.
const BEFORE_STEP_TOKENS: &str = r#"
PRAGMA foreign_keys = OFF;
BEGIN;
DROP TABLE secrets;
DROP TABLE invocation_prompts;
DROP TABLE revision_diffs;
DROP TABLE run_permissions;
DROP TABLE run_step_budgets;
ALTER TABLE runs DROP COLUMN limit_execution;
DROP TABLE step_outputs;
DROP TABLE money_step_budgets_before_tokens;
ALTER TABLE step_executions DROP COLUMN failure;
DROP TRIGGER resource_versions_no_update;
UPDATE resource_versions SET body = json_set(body, '$.body.steps[0].budget',
  json('{"micros": 2500000, "currency": "USD"}'));
CREATE TRIGGER resource_versions_no_update BEFORE UPDATE ON resource_versions
BEGIN SELECT RAISE(ABORT, 'published resource versions are immutable'); END;
INSERT INTO resource_drafts (resource_id, body, based_on, updated_at)
  SELECT resource_id, json_set(body, '$.body.steps[0].budget', json('{"micros": 1000000, "currency": "EUR"}')), number,
    100
  FROM resource_versions WHERE resource_id IN (SELECT id FROM resources WHERE kind = 'pipeline');
PRAGMA user_version = 9;
COMMIT;
"#;

#[test]
fn step_outputs_migration_reads_money_step_budgets_as_none_and_keeps_them() {
  let (repo, db, run, _) = started_run();
  let pipeline = run.snapshot().pipeline();
  drop(db);
  let database = repo.path().join(".orchester").join("orchester.db");
  Connection::open(&database)
    .expect("open")
    .execute_batch(BEFORE_STEP_TOKENS)
    .expect("back to money step budgets");

  let db = ProjectDb::open(repo.path()).expect("migrate");
  let version = db
    .version::<Pipeline>(pipeline.resource, pipeline.number)
    .expect("read")
    .expect("published version");
  assert_eq!(version.body().steps()[0].budget, None);
  let draft = db.draft::<Pipeline>(pipeline.resource).expect("read").expect("draft");
  assert_eq!(draft.body().steps()[0].budget, None);
  // Runs pinned the version before; their snapshots still read.
  assert!(db.run(run.id()).expect("read").is_some());
  drop(db);

  let connection = Connection::open(&database).expect("open");
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
        pipeline.resource.to_string(),
        "implement".to_owned(),
        1_000_000,
        "EUR".to_owned()
      ),
      (
        "version".to_owned(),
        pipeline.version.to_string(),
        "implement".to_owned(),
        2_500_000,
        "USD".to_owned()
      ),
    ]
  );
}
