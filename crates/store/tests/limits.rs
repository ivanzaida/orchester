//! Token budgets in the project database: Project settings › Limits, the project's daily usage and
//! the migration of money budgets to tokens.

mod support;

use std::time::Duration;

use orchester_core::{
  connection::stream::AgentEvent,
  limits::ProjectLimits,
  run::{PauseReason, Run, RunStatus, TokenUsage},
  store::{ActivityStore, RunStore, SettingsStore, TaskStore},
  task::Task,
  tokens::TokensUsed,
};
use orchester_store::ProjectDb;
use rusqlite::Connection;

use self::support::{at, budget, model, started_run};

#[test]
fn limits_default_until_saved_then_round_trip() {
  let (repo, mut db, ..) = started_run();
  assert_eq!(db.project_limits().expect("limits"), ProjectLimits::DEFAULT);
  let changed = ProjectLimits::new(
    budget(8_000_000),
    budget(2_000_000),
    budget(30_000_000),
    Duration::from_secs(45 * 60),
  )
  .expect("valid limits");
  db.save_project_limits(&changed).expect("save");
  db.save_project_limits(&changed).expect("save again");
  let reopened = ProjectDb::open(repo.path()).expect("open");
  assert_eq!(reopened.project_limits().expect("limits"), changed);
}

#[test]
fn limits_daily_usage_counts_completed_turns_since_the_day_began() {
  let (_repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("start");
  let turn = |input, cached, output| AgentEvent::TurnCompleted {
    usage: TokenUsage {
      input,
      cached_input: cached,
      output,
    },
  };
  db.append_activity(
    &invocation,
    &[
      (at(100), turn(12_000, 9_000, 500)),
      (at(200), AgentEvent::TurnStarted),
      (at(300), turn(20_000, 0, 1_000)),
    ],
  )
  .expect("append");
  assert_eq!(db.tokens_used_since(at(0)).expect("usage"), TokensUsed::new(24_500));
  assert_eq!(db.tokens_used_since(at(150)).expect("usage"), TokensUsed::new(21_000));
  assert_eq!(db.tokens_used_since(at(301)).expect("usage"), TokensUsed::ZERO);
}

/// Rewrites a migrated project database back into its shape before token budgets (schema 8):
/// money budget columns, events of schema 1 and no limits table.
const MONEY_SCHEMA: &str = r#"
PRAGMA foreign_keys = OFF;
BEGIN;
DROP TABLE step_outputs;
DROP TABLE money_step_budgets_before_tokens;
ALTER TABLE step_executions DROP COLUMN failure;
DROP TABLE project_limits;
DROP TABLE money_budgets_before_tokens;
DROP INDEX agent_events_by_kind;
CREATE TABLE tasks_money (
  id TEXT PRIMARY KEY,
  number INTEGER NOT NULL UNIQUE CHECK (number > 0),
  batch_key TEXT NOT NULL,
  title TEXT NOT NULL,
  brief TEXT NOT NULL,
  pipeline_id TEXT,
  budget_micros INTEGER,
  budget_currency TEXT,
  max_duration_ms INTEGER,
  status TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  CHECK ((budget_micros IS NULL) = (budget_currency IS NULL)),
  CHECK ((budget_micros IS NULL) = (max_duration_ms IS NULL))
) STRICT;
INSERT INTO tasks_money SELECT id, number, batch_key, title, brief, pipeline_id,
  CASE WHEN budget_tokens IS NULL THEN NULL ELSE 5000000 END,
  CASE WHEN budget_tokens IS NULL THEN NULL ELSE 'USD' END, max_duration_ms, status, created_at FROM tasks;
DROP TABLE tasks;
ALTER TABLE tasks_money RENAME TO tasks;
CREATE INDEX tasks_by_batch ON tasks (batch_key);
CREATE TABLE runs_money (
  id TEXT PRIMARY KEY,
  number INTEGER NOT NULL UNIQUE CHECK (number > 0),
  idempotency_key TEXT NOT NULL UNIQUE,
  task_id TEXT NOT NULL REFERENCES tasks (id),
  pipeline_version_id TEXT NOT NULL,
  snapshot TEXT NOT NULL,
  branch TEXT NOT NULL,
  base_revision TEXT NOT NULL CHECK (length(base_revision) = 40),
  currency TEXT NOT NULL,
  budget_micros INTEGER NOT NULL CHECK (budget_micros > 0),
  spent_micros INTEGER NOT NULL CHECK (spent_micros >= 0),
  status TEXT NOT NULL
    CHECK (status IN ('pending', 'running', 'paused', 'interrupted', 'succeeded', 'failed', 'cancelled')),
  status_reason TEXT CHECK ((status IN ('interrupted', 'failed')) = (status_reason IS NOT NULL)),
  failed_step TEXT CHECK ((status = 'failed') = (failed_step IS NOT NULL)),
  last_sequence INTEGER NOT NULL CHECK (last_sequence > 0),
  created_at INTEGER NOT NULL
) STRICT;
INSERT INTO runs_money SELECT id, number, idempotency_key, task_id, pipeline_version_id, snapshot, branch,
  base_revision, 'USD', 5000000, 0, status, CASE status WHEN 'paused' THEN NULL ELSE status_reason END, failed_step,
  last_sequence, created_at FROM runs;
DROP TABLE runs;
ALTER TABLE runs_money RENAME TO runs;
CREATE INDEX runs_by_task ON runs (task_id, number);
CREATE INDEX runs_by_status ON runs (status, number);
DROP TRIGGER run_events_no_update;
UPDATE run_events SET payload = json_set(json_remove(payload, '$.body.max_duration'), '$.schema', 1,
  '$.body.budget', json('{"micros": 5000000, "currency": "USD"}')) WHERE kind = 'created';
UPDATE run_events SET payload = json_set(json_remove(payload, '$.body.reason'), '$.schema', 1) WHERE kind = 'paused';
UPDATE run_events SET payload = json_set(payload, '$.schema', 1) WHERE kind NOT IN ('created', 'paused');
CREATE TRIGGER run_events_no_update BEFORE UPDATE ON run_events
BEGIN SELECT RAISE(ABORT, 'run events are append-only'); END;
PRAGMA user_version = 8;
COMMIT;
"#;

#[test]
fn limits_migration_converts_money_budgets_to_tokens_and_keeps_the_money_amounts() {
  let (repo, mut db, mut run, execution) = started_run();
  let stored = db.task(run.task()).expect("read").expect("task");
  let mut details = stored.details().clone();
  details.acceptance_criteria = vec!["Says hello.".to_owned()];
  let task = Task::from_stored(
    stored.id(),
    stored.key().clone(),
    details,
    stored.status(),
    stored.created_at(),
  );
  db.save_task(&task).expect("criteria");
  let mut invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("start");
  invocation.usage = TokenUsage {
    input: 12_166,
    cached_input: 2_432,
    output: 5,
  };
  db.save_invocation(&invocation).expect("save");
  let events = run.pause(at(20)).expect("pause");
  db.append_events(&run, &events).expect("append");
  drop(db);
  let database = repo.path().join(".orchester").join("orchester.db");
  Connection::open(&database)
    .expect("open")
    .execute_batch(MONEY_SCHEMA)
    .expect("back to money budgets");

  let db = ProjectDb::open(repo.path()).expect("migrate");
  let migrated_task = db.task(task.id()).expect("read").expect("task");
  let limits = migrated_task.details().limits.expect("limits kept");
  assert_eq!(limits.budget(), budget(1_400_000));
  assert_eq!(limits.max_duration(), Duration::from_secs(1_800));
  assert_eq!(migrated_task.details().title, task.details().title);
  assert_eq!(migrated_task.details().acceptance_criteria, ["Says hello."]);
  let migrated = db.run(run.id()).expect("read").expect("run");
  assert_eq!(migrated.status(), &RunStatus::Paused(PauseReason::Requested));
  assert_eq!(migrated.budget(), budget(1_400_000));
  assert_eq!(migrated.created().budget, budget(1_400_000));
  assert_eq!(migrated.max_duration(), Duration::from_secs(1_800));
  // 12,166 input − 2,432 cached + 5 output.
  assert_eq!(migrated.used(), TokensUsed::new(9_739));
  assert_eq!(migrated.attempts(), run.attempts());
  let replayed = Run::replay(&db.events(run.id()).expect("events")).expect("the old log still reads");
  assert_eq!(replayed.status(), migrated.status());
  assert_eq!(replayed.budget(), migrated.budget());
  assert_eq!(db.invocations(run.id()).expect("invocations").len(), 1);
  drop(db);

  let connection = Connection::open(&database).expect("open");
  let mut kept = connection
    .prepare("SELECT owner, micros, currency FROM money_budgets_before_tokens ORDER BY owner")
    .expect("prepare");
  let kept: Vec<(String, i64, String)> = kept
    .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
    .expect("query")
    .collect::<Result<_, _>>()
    .expect("rows");
  assert_eq!(
    kept,
    [
      ("run".to_owned(), 5_000_000, "USD".to_owned()),
      ("task".to_owned(), 5_000_000, "USD".to_owned())
    ]
  );
  let version: u32 = connection
    .pragma_query_value(None, "user_version", |row| row.get(0))
    .expect("version");
  assert_eq!(version, 10);
}
