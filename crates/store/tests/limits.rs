//! Token budgets in the project database: Project settings › Limits, the project's daily usage and
//! the migration of money budgets to tokens.

mod support;

use std::{fmt::Debug, num::NonZeroU32, path::Path, str::FromStr, time::Duration};

use orchester_core::{
  connection::stream::AgentEvent,
  limits::{AgentSlots, ProjectLimits},
  resource::StepKey,
  run::{Attempt, AttemptReason, AttemptStatus, PauseReason, Run, RunStatus, StepExecution, StepStatus, TokenUsage},
  store::{ActivityStore, RunStore, SettingsStore, TaskStore},
  tokens::TokensUsed,
};
use orchester_store::ProjectDb;
use rusqlite::Connection;

use self::support::{at, budget, copied_at_version, model, started_run};

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
fn limits_agent_slots_persist_with_the_other_limits() {
  let (repo, mut db, ..) = started_run();
  let two = AgentSlots::new(2).expect("in range");
  let changed = ProjectLimits::DEFAULT.with_agent_slots(two);
  db.save_project_limits(&changed).expect("save");
  let reopened = ProjectDb::open(repo.path()).expect("open");
  assert_eq!(reopened.project_limits().expect("limits").agent_slots(), two);
  assert_eq!(reopened.project_limits().expect("limits"), changed);
}

#[test]
fn limits_migration_gives_saved_limits_the_default_agent_slots() {
  let (current, mut db, ..) = started_run();
  let saved = ProjectLimits::new(
    budget(8_000_000),
    budget(2_000_000),
    budget(30_000_000),
    Duration::from_secs(45 * 60),
  )
  .expect("valid limits");
  db.save_project_limits(&saved).expect("save");
  drop(db);
  let database = current.path().join(".orchester").join("orchester.db");
  let before = schema_version(&database) - 1;
  let old = copied_at_version(&database, before);
  let old_database = old.path().join(".orchester").join("orchester.db");
  let columns: Vec<String> = Connection::open(&old_database)
    .expect("open")
    .prepare("SELECT name FROM pragma_table_info('project_limits')")
    .expect("columns")
    .query_map([], |row| row.get(0))
    .expect("columns")
    .collect::<Result<_, _>>()
    .expect("columns");
  assert!(!columns.iter().any(|column| column == "agent_slots"), "{columns:?}");

  let mut upgraded = ProjectDb::open(old.path()).expect("migrate");
  assert_eq!(upgraded.project_limits().expect("limits"), saved);
  assert_eq!(
    upgraded.project_limits().expect("limits").agent_slots(),
    AgentSlots::DEFAULT
  );
  let sixteen = saved.with_agent_slots(AgentSlots::new(16).expect("in range"));
  upgraded.save_project_limits(&sixteen).expect("save");
  assert_eq!(upgraded.project_limits().expect("limits"), sixteen);
  assert_eq!(schema_version(&old_database), schema_version(&database));
  let out_of_range = Connection::open(&old_database)
    .expect("open")
    .execute("UPDATE project_limits SET agent_slots = 17", []);
  assert!(out_of_range.is_err(), "the schema keeps agent slots within 1 to 16");
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

/// A project database at schema 8, before budgets were tokens (see the file).
const SCHEMA_8: &str = include_str!("fixtures/schema_8_money_budgets.sql");
const TASK: &str = "01920000-0000-7000-8000-000000000002";
const RUN: &str = "01920000-0000-7000-8000-000000000009";
const ATTEMPT: &str = "01920000-0000-7000-8000-00000000000b";
const EXECUTION: &str = "01920000-0000-7000-8000-00000000000c";

fn id<T: FromStr<Err: Debug>>(value: &str) -> T {
  value.parse().expect("fixture id")
}

fn schema_version(database: &Path) -> u32 {
  Connection::open(database)
    .expect("open")
    .pragma_query_value(None, "user_version", |row| row.get(0))
    .expect("version")
}

#[test]
fn limits_migration_converts_money_budgets_to_tokens_and_keeps_the_money_amounts() {
  let repo = tempfile::tempdir().expect("temporary directory");
  ProjectDb::create_at_version(repo.path(), 8)
    .expect("schema 8")
    .execute_batch(SCHEMA_8)
    .expect("rows of schema 8");

  let db = ProjectDb::open(repo.path()).expect("migrate");
  let task = db.task(id(TASK)).expect("read").expect("task");
  let limits = task.details().limits.expect("limits kept");
  assert_eq!(limits.budget(), budget(1_400_000));
  assert_eq!(limits.max_duration(), Duration::from_secs(1_800));
  assert_eq!(task.details().title.as_str(), "Say hello");
  assert_eq!(task.details().acceptance_criteria, ["Says hello."]);
  let run = db.run(id(RUN)).expect("read").expect("run");
  assert_eq!(run.status(), &RunStatus::Paused(PauseReason::Requested));
  assert_eq!(run.budget(), budget(1_400_000));
  assert_eq!(run.created().budget, budget(1_400_000));
  assert_eq!(run.max_duration(), Duration::from_secs(1_800));
  // 12,166 input − 2,432 cached + 5 output.
  assert_eq!(run.used(), TokensUsed::new(9_739));
  let implement = StepExecution::new(
    id(EXECUTION),
    StepKey::new("implement").expect("key"),
    StepStatus::Pending,
  );
  let attempt = Attempt::new(
    id(ATTEMPT),
    NonZeroU32::MIN,
    AttemptReason::Initial,
    AttemptStatus::Active,
    vec![implement],
  );
  assert_eq!(run.attempts(), [attempt]);
  let replayed = Run::replay(&db.events(run.id()).expect("events")).expect("the old log still reads");
  assert_eq!(replayed.status(), run.status());
  assert_eq!(replayed.budget(), run.budget());
  assert_eq!(db.invocations(run.id()).expect("invocations").len(), 1);
  drop(db);

  let database = repo.path().join(".orchester").join("orchester.db");
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
  // Migrated all the way: the same version a new project database starts at.
  let (fresh, ..) = started_run();
  assert_eq!(
    schema_version(&database),
    schema_version(&fresh.path().join(".orchester").join("orchester.db"))
  );
}

#[test]
fn limits_update_changes_what_is_stored_in_one_transaction() {
  let (repo, mut db, ..) = started_run();
  let changed = db
    .update_project_limits(|stored| {
      assert_eq!(stored, ProjectLimits::DEFAULT);
      ProjectLimits::new(
        stored.max_run_budget(),
        stored.default_run_budget(),
        budget(30_000_000),
        stored.default_time_limit(),
      )
      .ok()
    })
    .expect("update")
    .expect("saved");
  assert_eq!(changed.0, ProjectLimits::DEFAULT, "the limits before");
  assert_eq!(changed.1.daily_budget(), budget(30_000_000));
  // A change that saves nothing leaves the limits as they are.
  assert_eq!(db.update_project_limits(|_| None).expect("update"), None);
  let reopened = ProjectDb::open(repo.path()).expect("open");
  assert_eq!(reopened.project_limits().expect("limits"), changed.1);
}
