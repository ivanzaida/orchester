//! A project with a started run, shared by the tests of the execution tables.

#![allow(dead_code, reason = "shared by several test binaries, each of which uses a part")]

use std::{collections::BTreeMap, fs, time::Duration};

use orchester_core::{
  id::{ConnectionId, IdempotencyKey, ProjectId, StepExecutionId},
  name::Name,
  permission::StepPermissions,
  project::{Project, ProjectKey},
  resource::{
    AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step, StepKey, StepKind,
  },
  revision::Revision,
  run::{BranchName, NewRun, PublishedVersions, Run, RunSnapshot},
  store::{ResourceStore, RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_store::ProjectDb;
use tempfile::TempDir;

pub fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

pub fn name(value: &str) -> Name {
  Name::new(value).expect("valid name")
}

pub fn model() -> ModelId {
  ModelId::new("gpt-5.5").expect("valid model")
}

/// A project with a started run of a one-step pipeline.
pub fn started_run() -> (TempDir, ProjectDb, Run, StepExecutionId) {
  run_of_one_step(false, limits())
}

/// [`started_run`] whose step waits for a plan decision when it finishes.
pub fn started_gated_run() -> (TempDir, ProjectDb, Run, StepExecutionId) {
  run_of_one_step(true, limits())
}

/// [`started_run`] of a task, and so a run, without a token budget or a time limit.
pub fn started_run_without_limits() -> (TempDir, ProjectDb, Run, StepExecutionId) {
  run_of_one_step(false, TaskLimits::NONE)
}

/// A recorded run of [`started_run`]'s one step that has not started.
pub fn pending_run() -> (TempDir, ProjectDb, Run) {
  recorded_run(false, limits())
}

/// 1M tokens and 30 minutes.
fn limits() -> TaskLimits {
  TaskLimits::new(Some(budget(1_000_000)), Some(Duration::from_secs(1_800))).expect("limits")
}

fn run_of_one_step(approval_gate: bool, limits: TaskLimits) -> (TempDir, ProjectDb, Run, StepExecutionId) {
  let (repo, mut db, mut run) = recorded_run(approval_gate, limits);
  let events = run.start(at(7)).expect("start");
  db.append_events(&run, &events).expect("append");
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  (repo, db, run, execution)
}

fn recorded_run(approval_gate: bool, limits: TaskLimits) -> (TempDir, ProjectDb, Run) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("key"),
    name("Kontur"),
  );
  let mut db = ProjectDb::create(repo.path(), project).expect("create project");
  let details = TaskDetails {
    title: name("Say hello"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits,
  };
  let task = db
    .create_tasks(IdempotencyKey::generate(), vec![details], at(1))
    .expect("task")[0]
    .id();
  let profile = AgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    ConnectionId::generate(),
    model(),
    ProviderSettings::default(),
  );
  let builder = db
    .create_resource(Slug::new("builder").expect("slug"), name("Builder"), &profile, at(2))
    .expect("profile")
    .id();
  let profiles = BTreeMap::from([(
    builder,
    db.publish_draft::<AgentProfile>(builder, at(3)).expect("publish"),
  )]);
  let step = Step {
    key: StepKey::new("implement").expect("key"),
    name: name("Implement"),
    kind: StepKind::Agent {
      profile: Some(builder),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("prompt"),
    depends_on: Default::default(),
    approval_gate,
    on_failure: None,
    budget: None,
  };
  let pipeline = Pipeline::new(vec![step]).expect("pipeline");
  let delivery = db
    .create_resource(Slug::new("delivery").expect("slug"), name("Delivery"), &pipeline, at(4))
    .expect("pipeline")
    .id();
  let version = db.publish_draft::<Pipeline>(delivery, at(5)).expect("publish");
  let (tools, instructions) = (BTreeMap::new(), BTreeMap::new());
  let published = PublishedVersions {
    profiles: &profiles,
    tools: &tools,
    instructions: &instructions,
    skills: &BTreeMap::new(),
  };
  let new = NewRun {
    task,
    snapshot: RunSnapshot::build(delivery, &version, published).expect("snapshot"),
    branch: BranchName::new("orchester/orc-1-run-1").expect("branch"),
    base: Revision::new(&"a".repeat(40)).expect("revision"),
    base_branch: None,
    base_source: orchester_core::integration::base::RunBaseSource::CheckedOut,
    budget: limits.budget(),
    max_duration: limits.max_duration(),
  };
  let run = db.create_run(IdempotencyKey::generate(), new, at(6)).expect("run");
  (repo, db, run)
}

pub fn budget(tokens: u64) -> TokenBudget {
  TokenBudget::new(tokens).expect("above zero")
}

/// A project database at schema version `version` in a repository of its own, holding the rows of
/// `current` (a database at the latest schema) in the columns both have.
pub fn copied_at_version(current: &std::path::Path, version: u32) -> tempfile::TempDir {
  let repo = tempfile::tempdir().expect("temporary directory");
  std::fs::create_dir(repo.path().join(".git")).expect("create .git");
  let old = ProjectDb::create_at_version(repo.path(), version).expect("an old database");
  old
    .execute(
      "ATTACH DATABASE ?1 AS current",
      [current.to_str().expect("a UTF-8 path")],
    )
    .expect("attach");
  old
    .pragma_update(None, "foreign_keys", false)
    .expect("no references while copying");
  let tables: Vec<String> = old
    .prepare("SELECT name FROM main.sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'")
    .expect("tables")
    .query_map([], |row| row.get(0))
    .expect("tables")
    .collect::<Result<_, _>>()
    .expect("tables");
  for table in tables {
    // The columns both versions have: a later migration may also have dropped one, or the whole table.
    let columns: Vec<String> = old
      .prepare(&format!(
        "SELECT name FROM pragma_table_info('{table}')          WHERE name IN (SELECT name FROM pragma_table_info('{table}', 'current'))"
      ))
      .expect("columns")
      .query_map([], |row| row.get(0))
      .expect("columns")
      .collect::<Result<_, _>>()
      .expect("columns");
    if columns.is_empty() {
      continue;
    }
    let columns = columns.join(", ");
    // Replacing: the migrations seed some rows themselves, such as the counters.
    old
      .execute(
        &format!("INSERT OR REPLACE INTO main.{table} ({columns}) SELECT {columns} FROM current.{table}"),
        [],
      )
      .expect("copy the rows");
  }
  drop(old);
  repo
}
