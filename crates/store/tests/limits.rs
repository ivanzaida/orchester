//! Token budgets in the project database: Project settings › Limits, the project's daily usage and
//! the migration of money budgets to tokens.

mod support;

use std::{fmt::Debug, num::NonZeroU32, path::Path, str::FromStr, time::Duration};

use orchester_core::{
  connection::stream::AgentEvent,
  id::IdempotencyKey,
  limits::{AgentSlots, ProjectLimits},
  resource::StepKey,
  run::{
    Attempt, AttemptReason, AttemptStatus, PauseReason, Run, RunLimit, RunStatus, StepExecution, StepStatus, TokenUsage,
  },
  store::{ActivityStore, RunStore, SettingsStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  tokens::TokensUsed,
};
use orchester_store::ProjectDb;
use rusqlite::Connection;

use self::support::{at, budget, copied_at_version, model, name, started_run, started_run_without_limits};

#[test]
fn limits_default_until_saved_then_round_trip() {
  let (repo, mut db, ..) = started_run();
  assert_eq!(db.project_limits().expect("limits"), ProjectLimits::DEFAULT);
  let changed = ProjectLimits::new(
    Some(budget(2_000_000)),
    budget(30_000_000),
    Some(Duration::from_secs(45 * 60)),
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
  let (current, db, ..) = started_run();
  drop(db);
  let database = current.path().join(".orchester").join("orchester.db");
  let before = version_before_agent_slots(schema_version(&database));
  let old = copied_at_version(&database, before);
  let old_database = old.path().join(".orchester").join("orchester.db");
  assert!(!has_agent_slots(&Connection::open(&old_database).expect("open")));
  insert_limits_with_maximum(&old_database, "");

  let mut upgraded = ProjectDb::open(old.path()).expect("migrate");
  let saved = ProjectLimits::new(
    Some(budget(2_000_000)),
    budget(30_000_000),
    Some(Duration::from_secs(45 * 60)),
  )
  .expect("valid limits");
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
fn limits_migration_keeps_every_limit_and_drops_the_maximum() {
  let (current, mut db, run, _) = started_run();
  let unlimited = db
    .create_tasks(
      IdempotencyKey::generate(),
      vec![TaskDetails {
        title: name("Without limits"),
        brief: String::new(),
        acceptance_criteria: Vec::new(),
        pipeline: None,
        limits: TaskLimits::NONE,
      }],
      at(8),
    )
    .expect("task")[0]
    .id();
  drop(db);
  let database = current.path().join(".orchester").join("orchester.db");
  let before = schema_version(&database) - 1;
  let old = copied_at_version(&database, before);
  let old_database = old.path().join(".orchester").join("orchester.db");
  assert!(has_maximum(&Connection::open(&old_database).expect("open")));
  insert_limits_with_maximum(&old_database, ", 3");
  Connection::open(&old_database)
    .expect("open")
    .execute(
      "INSERT INTO run_step_budgets (run_id, step_key, budget_tokens) VALUES (?1, 'implement', 600000)",
      [run.id().to_string()],
    )
    .expect("a raised step budget");

  let mut upgraded = ProjectDb::open(old.path()).expect("migrate");
  assert!(!has_maximum(&Connection::open(&old_database).expect("open")));
  let limits = ProjectLimits::new(
    Some(budget(2_000_000)),
    budget(30_000_000),
    Some(Duration::from_secs(45 * 60)),
  )
  .expect("valid limits")
  .with_agent_slots(AgentSlots::new(3).expect("in range"));
  assert_eq!(upgraded.project_limits().expect("limits"), limits);
  let task = upgraded.task(run.task()).expect("read").expect("task");
  let kept = TaskLimits::new(Some(budget(1_000_000)), Some(Duration::from_secs(1_800))).expect("limits");
  assert_eq!(task.details().limits, kept);
  let task = upgraded.task(unlimited).expect("read").expect("task");
  assert_eq!(task.details().limits, TaskLimits::NONE, "no limits stay no limits");
  let read = upgraded.run(run.id()).expect("read").expect("run");
  assert_eq!(read.budget(), Some(budget(1_000_000)));
  assert_eq!(read.max_duration(), Some(Duration::from_secs(1_800)));
  assert_eq!(read.created().budget, Some(budget(1_000_000)));
  let step = StepKey::new("implement").expect("key");
  assert_eq!(read.step_budget(&step), Some(budget(600_000)));

  // After the migration each limit is optional on its own.
  let mut details = task.details().clone();
  details.limits = TaskLimits::new(Some(budget(5_000)), None).expect("limits");
  let saved = upgraded
    .update_task(unlimited, task.revision(), details.clone())
    .expect("save");
  assert_eq!(saved.saved().expect("no conflict").details().limits, details.limits);
  let no_default = ProjectLimits::new(None, budget(30_000_000), None).expect("valid limits");
  upgraded.save_project_limits(&no_default).expect("save");
  assert_eq!(upgraded.project_limits().expect("limits"), no_default);
}

#[test]
fn limits_a_run_without_limits_and_a_lifted_step_budget_round_trip() {
  let (repo, mut db, mut run, execution) = started_run_without_limits();
  let stored = db.run(run.id()).expect("read").expect("run");
  assert_eq!((stored.budget(), stored.max_duration()), (None, None));
  assert_eq!(stored.created().budget, None);
  let limit = RunLimit::StepBudget { execution };
  let events = run.stop_at_limit(limit, at(8)).expect("stop");
  db.append_events(&run, &events).expect("append");
  let events = run
    .raise_step_budget(None, TokensUsed::new(400_000), at(9))
    .expect("lift");
  db.append_events(&run, &events).expect("append");
  let reopened = ProjectDb::open(repo.path()).expect("open");
  let stored = reopened.run(run.id()).expect("read").expect("run");
  assert_eq!(stored.step_budget(&StepKey::new("implement").expect("key")), None);
  assert_eq!(stored.raised_step_budgets().values().collect::<Vec<_>>(), [&None]);
  assert_eq!(stored, run);
  let events = reopened.events(run.id()).expect("events");
  assert_eq!(Run::replay(&events), Ok(run));
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

fn has_agent_slots(connection: &Connection) -> bool {
  connection
    .prepare("SELECT 1 FROM pragma_table_info('project_limits') WHERE name = 'agent_slots'")
    .expect("columns")
    .exists([])
    .expect("columns")
}

/// The newest schema without the agent slots column, wherever later migrations moved the latest.
fn version_before_agent_slots(latest: u32) -> u32 {
  (1..latest)
    .rev()
    .find(|&version| {
      let repo = tempfile::tempdir().expect("temporary directory");
      !has_agent_slots(&ProjectDb::create_at_version(repo.path(), version).expect("an old database"))
    })
    .expect("a schema before the agent slots")
}

fn has_maximum(connection: &Connection) -> bool {
  connection
    .prepare("SELECT 1 FROM pragma_table_info('project_limits') WHERE name = 'max_run_budget'")
    .expect("columns")
    .exists([])
    .expect("columns")
}

/// Project settings › Limits as a schema with a maximum run budget stored them: at most 8M per run,
/// 2M by default, 30M a day and 45 minutes; `slots` adds the agent slots column's value when the
/// schema has it (`", 3"`).
fn insert_limits_with_maximum(database: &Path, slots: &str) {
  let columns = if slots.is_empty() { "" } else { ", agent_slots" };
  Connection::open(database)
    .expect("open")
    .execute(
      &format!(
        "INSERT INTO project_limits (id, max_run_budget, default_run_budget, daily_budget, \
         default_time_limit_ms{columns}) VALUES (1, 8000000, 2000000, 30000000, 2700000{slots})"
      ),
      [],
    )
    .expect("the old limits");
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
  let limits = task.details().limits;
  assert_eq!(limits.budget(), Some(budget(1_400_000)));
  assert_eq!(limits.max_duration(), Some(Duration::from_secs(1_800)));
  assert_eq!(task.details().title.as_str(), "Say hello");
  assert_eq!(task.details().acceptance_criteria, ["Says hello."]);
  let run = db.run(id(RUN)).expect("read").expect("run");
  assert_eq!(run.status(), &RunStatus::Paused(PauseReason::Requested));
  assert_eq!(run.budget(), Some(budget(1_400_000)));
  assert_eq!(run.created().budget, Some(budget(1_400_000)));
  assert_eq!(run.max_duration(), Some(Duration::from_secs(1_800)));
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
