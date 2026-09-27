use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  num::NonZeroU32,
  path::Path,
};

use orchester_core::{
  id::{ConnectionId, IdempotencyKey, ProjectId, ResourceId, TaskId},
  name::Name,
  permission::StepPermissions,
  project::{Project, ProjectKey},
  resource::{
    AgentProfile, FailureRoute, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step, StepKey,
    StepKind,
  },
  revision::Revision,
  run::{
    BranchName, InterruptCause, NewRun, PauseReason, PlanDecision, PublishedVersions, Run, RunEvent, RunLimit,
    RunSnapshot, RunStatus, StepExecutor, StepFailure, StepStatus,
  },
  store::{ResourceStore, RunStore, TaskStore},
  task::TaskDetails,
  time::Timestamp,
  tokens::{TokenBudget, TokensUsed},
};
use orchester_store::{ProjectDb, RunStoreError, StoreError};
use rusqlite::Connection;
use tempfile::TempDir;

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

fn name(value: &str) -> Name {
  Name::new(value).expect("valid name")
}

fn key(value: &str) -> StepKey {
  StepKey::new(value).expect("valid key")
}

fn revision(digit: char) -> Revision {
  Revision::new(&digit.to_string().repeat(40)).expect("valid revision")
}

fn raw(repo: &Path) -> Connection {
  Connection::open(repo.join(".orchester/orchester.db")).expect("open raw database")
}

fn step(value: &str, profile: ResourceId, depends_on: &[&str]) -> Step {
  Step {
    key: key(value),
    name: name(value),
    kind: StepKind::Agent {
      profile: Some(profile),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("valid prompt"),
    depends_on: depends_on.iter().map(|dependency| key(dependency)).collect(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }
}

/// A project with one task and a published plan (gated) → implement → checks pipeline whose
/// checks return to implement once; implement overrides the profile's model.
struct Fixture {
  repo: TempDir,
  db: ProjectDb,
  task: TaskId,
  snapshot: RunSnapshot,
}

impl Fixture {
  fn new() -> Self {
    let repo = tempfile::tempdir().expect("temporary directory");
    fs::create_dir(repo.path().join(".git")).expect("create .git");
    let project = Project::new(
      ProjectId::generate(),
      ProjectKey::new("ORC").expect("valid key"),
      name("Kontur"),
    );
    let mut db = ProjectDb::create(repo.path(), project).expect("create project");
    let task = Self::task(&mut db);
    let snapshot = Self::publish(&mut db);
    Self {
      repo,
      db,
      task,
      snapshot,
    }
  }

  fn task(db: &mut ProjectDb) -> TaskId {
    let details = TaskDetails {
      title: name("Add repository switcher"),
      brief: String::new(),
      acceptance_criteria: Vec::new(),
      pipeline: None,
      limits: None,
    };
    db.create_tasks(IdempotencyKey::generate(), vec![details], at(1))
      .expect("create task")[0]
      .id()
  }

  fn publish(db: &mut ProjectDb) -> RunSnapshot {
    let profile = AgentProfile::new(
      name("Builder"),
      String::new(),
      PromptText::new("Build.").expect("valid prompt"),
      ConnectionId::generate(),
      ModelId::new("model-b").expect("valid model"),
      ProviderSettings::default(),
    );
    let builder = db
      .create_resource(Slug::new("builder").expect("slug"), name("Builder"), &profile, at(2))
      .expect("create profile")
      .id();
    let profile_version = db
      .publish_draft::<AgentProfile>(builder, at(3))
      .expect("publish profile");
    let implement = Step {
      kind: StepKind::Agent {
        profile: Some(builder),
        repository: RepositoryAccess::Write,
        model: Some(ModelId::new("model-override").expect("valid model")),
        permissions: StepPermissions::default(),
      },
      ..step("implement", builder, &["plan"])
    };
    let checks = Step {
      on_failure: Some(FailureRoute {
        return_to: key("implement"),
        max_retries: NonZeroU32::MIN,
      }),
      ..step("checks", builder, &["implement"])
    };
    let plan = Step {
      approval_gate: true,
      ..step("plan", builder, &[])
    };
    let pipeline = Pipeline::new(vec![plan, implement, checks]).expect("valid pipeline");
    let delivery = db
      .create_resource(Slug::new("delivery").expect("slug"), name("Delivery"), &pipeline, at(4))
      .expect("create pipeline")
      .id();
    let pipeline_version = db.publish_draft::<Pipeline>(delivery, at(5)).expect("publish pipeline");
    let profiles = BTreeMap::from([(builder, profile_version)]);
    let (tools, instructions) = (BTreeMap::new(), BTreeMap::new());
    let published = PublishedVersions {
      profiles: &profiles,
      tools: &tools,
      instructions: &instructions,
    };
    RunSnapshot::build(delivery, &pipeline_version, published).expect("published references")
  }

  fn new_run(&self) -> NewRun {
    NewRun {
      task: self.task,
      snapshot: self.snapshot.clone(),
      branch: BranchName::new("orchester/orc-1").expect("valid branch"),
      base: revision('a'),
      budget: TokenBudget::new(1_400_000).expect("above zero"),
      max_duration: std::time::Duration::from_secs(1_800),
    }
  }

  fn create(&mut self) -> Run {
    let new = self.new_run();
    self
      .db
      .create_run(IdempotencyKey::generate(), new, at(10))
      .expect("create run")
  }

  /// Applies an accepted transition to `run` and persists its events.
  fn apply(
    &mut self,
    run: &mut Run,
    transition: impl FnOnce(&mut Run) -> Result<Vec<RunEvent>, orchester_core::run::RunError>,
  ) {
    let events = transition(run).expect("transition is accepted");
    self.db.append_events(run, &events).expect("append events");
  }

  fn exec(run: &Run, step: &str) -> orchester_core::id::StepExecutionId {
    run.latest_execution(&key(step)).expect("execution").id()
  }

  /// Starts a run and takes it through an approved plan into a running implement step.
  fn implementing(&mut self) -> Run {
    let mut run = self.create();
    self.apply(&mut run, |run| run.start(at(11)));
    let plan = Self::exec(&run, "plan");
    self.apply(&mut run, |run| run.start_step(plan, at(12)));
    self.apply(&mut run, |run| run.complete_step(plan, at(13)));
    self.apply(&mut run, |run| run.decide_plan(plan, PlanDecision::Approved, at(14)));
    let implement = Self::exec(&run, "implement");
    self.apply(&mut run, |run| run.start_step(implement, at(15)));
    run
  }
}

#[test]
fn runs_are_created_with_consecutive_numbers_and_a_creation_event() {
  let mut fixture = Fixture::new();
  let first = fixture.create();
  let second = fixture.create();
  assert_eq!(first.number().get(), 1);
  assert_eq!(second.number().get(), 2);
  assert_eq!(first.status(), &RunStatus::Pending);
  assert_eq!(fixture.db.run(first.id()).expect("load"), Some(first.clone()));
  assert_eq!(
    fixture.db.runs_of_task(fixture.task).expect("list"),
    vec![first.clone(), second]
  );
  let events = fixture.db.events(first.id()).expect("events");
  assert_eq!(events.len(), 1);
  assert_eq!(events[0].kind().name(), "created");
}

#[test]
fn runs_creation_is_idempotent_per_key() {
  let mut fixture = Fixture::new();
  let key = IdempotencyKey::generate();
  let first = fixture.db.create_run(key, fixture.new_run(), at(10)).expect("create");
  let retry = fixture.db.create_run(key, fixture.new_run(), at(11)).expect("retry");
  assert_eq!(retry, first);
  assert_eq!(fixture.db.runs_of_task(fixture.task).expect("list").len(), 1);
  assert_eq!(fixture.create().number().get(), 2);
}

#[test]
fn runs_need_an_existing_task() {
  let mut fixture = Fixture::new();
  let mut new = fixture.new_run();
  new.task = TaskId::generate();
  let missing = new.task;
  assert!(matches!(
    fixture.db.create_run(IdempotencyKey::generate(), new, at(10)),
    Err(RunStoreError::Store(StoreError::TaskNotFound(task))) if task == missing
  ));
}

#[test]
fn runs_state_and_checkpoints_round_trip() {
  let mut fixture = Fixture::new();
  let mut run = fixture.implementing();
  let implement = Fixture::exec(&run, "implement");
  fixture.apply(&mut run, |run| run.record_checkpoint(implement, revision('b'), at(16)));
  fixture.apply(&mut run, |run| run.record_usage(TokensUsed::new(700), at(17)));
  let stored = fixture.db.run(run.id()).expect("load").expect("exists");
  assert_eq!(stored, run);
  assert_eq!(
    stored.latest_execution(&key("plan")).map(|e| e.status()),
    Some(StepStatus::Passed)
  );
  let checkpoint = fixture.db.latest_checkpoint(run.id()).expect("checkpoint");
  assert_eq!(checkpoint.as_ref(), run.last_checkpoint());
  assert_eq!(checkpoint.map(|c| c.revision().clone()), Some(revision('b')));
}

#[test]
fn runs_step_executions_store_the_effective_profile_version_and_model() {
  let mut fixture = Fixture::new();
  let run = fixture.implementing();
  let Some(StepExecutor::Agent { profile, .. }) = run.snapshot().step(&key("implement")).map(|s| s.executor()) else {
    panic!("implement is an agent step");
  };
  let row: (String, String) = raw(fixture.repo.path())
    .query_row(
      "SELECT profile_version_id, model FROM step_executions WHERE id = ?1",
      [Fixture::exec(&run, "implement").to_string()],
      |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .expect("execution row");
  assert_eq!(row, (profile.version.to_string(), "model-override".to_owned()));
}

#[test]
fn runs_replaying_stored_events_reproduces_the_stored_state() {
  let mut fixture = Fixture::new();
  let mut run = fixture.implementing();
  let implement = Fixture::exec(&run, "implement");
  fixture.apply(&mut run, |run| run.record_checkpoint(implement, revision('c'), at(20)));
  fixture.apply(&mut run, |run| run.complete_step(implement, at(21)));
  let checks = Fixture::exec(&run, "checks");
  fixture.apply(&mut run, |run| run.start_step(checks, at(22)));
  fixture.apply(&mut run, |run| {
    run.fail_step(checks, StepFailure::MergeConflict, at(23))
  });
  assert_eq!(run.attempts().len(), 2);
  let loaded = fixture.db.run(run.id()).expect("load").expect("exists");
  let failed = loaded.attempts()[0].execution(checks).expect("checks");
  assert_eq!(failed.failure(), Some(StepFailure::MergeConflict));
  fixture.apply(&mut run, |run| run.interrupt(InterruptCause::Crashed, at(24)));
  let stored = fixture.db.run(run.id()).expect("load").expect("exists");
  let events = fixture.db.events(run.id()).expect("events");
  assert_eq!(Run::replay(&events), Ok(stored));
}

#[test]
fn runs_a_stale_writer_gets_a_sequence_conflict() {
  let mut fixture = Fixture::new();
  let mut run = fixture.create();
  let mut stale = run.clone();
  fixture.apply(&mut run, |run| run.start(at(11)));
  let events = stale.cancel(at(12)).expect("cancel is valid on the stale copy");
  assert!(matches!(
    fixture.db.append_events(&stale, &events),
    Err(RunStoreError::SequenceConflict {
      expected: 4,
      found: 2,
      ..
    })
  ));
  assert_eq!(fixture.db.run(run.id()).expect("load"), Some(run));
}

#[test]
fn runs_append_rejects_events_that_do_not_match_the_state() {
  let mut fixture = Fixture::new();
  let mut run = fixture.create();
  let mut other = fixture.create();
  assert!(matches!(
    fixture.db.append_events(&run, &[]),
    Err(RunStoreError::EventsMismatch(_))
  ));
  let foreign = other.start(at(11)).expect("start");
  assert!(matches!(
    fixture.db.append_events(&run, &foreign),
    Err(RunStoreError::EventsMismatch(_))
  ));
  let mut events = run.start(at(11)).expect("start");
  events.pop();
  assert!(matches!(
    fixture.db.append_events(&run, &events),
    Err(RunStoreError::EventsMismatch(_))
  ));
}

#[test]
fn runs_events_and_checkpoints_are_immutable() {
  let mut fixture = Fixture::new();
  let mut run = fixture.implementing();
  let implement = Fixture::exec(&run, "implement");
  fixture.apply(&mut run, |run| run.record_checkpoint(implement, revision('d'), at(16)));
  let connection = raw(fixture.repo.path());
  for statement in [
    "UPDATE run_events SET at = 0",
    "DELETE FROM run_events",
    "UPDATE checkpoints SET revision = 'x'",
    "DELETE FROM checkpoints",
  ] {
    let error = connection.execute(statement, []).expect_err(statement);
    assert!(
      error.to_string().contains("immutable") || error.to_string().contains("append-only"),
      "{statement}: {error}"
    );
  }
}

#[test]
fn runs_a_damaged_row_isreported_as_corrupt() {
  let mut fixture = Fixture::new();
  let run = fixture.create();
  raw(fixture.repo.path())
    .execute(
      "UPDATE runs SET snapshot = '{\"schema\":1,\"body\":{}}' WHERE id = ?1",
      [run.id().to_string()],
    )
    .expect("tamper");
  assert!(matches!(
    fixture.db.run(run.id()),
    Err(RunStoreError::Store(StoreError::Corrupt {
      what: "run snapshot",
      ..
    }))
  ));
}

#[test]
fn runs_interrupted_by_runner_loss_continue_after_reconnect_once() {
  let mut fixture = Fixture::new();
  let mut lost = fixture.implementing();
  let mut crashed = fixture.implementing();
  let running = fixture.implementing();
  let implement = Fixture::exec(&lost, "implement");
  fixture.apply(&mut lost, |run| run.record_checkpoint(implement, revision('e'), at(16)));
  fixture.apply(&mut lost, |run| run.interrupt(InterruptCause::RunnerLost, at(17)));
  fixture.apply(&mut crashed, |run| run.interrupt(InterruptCause::Crashed, at(17)));
  let interrupted: Vec<_> = fixture
    .db
    .interrupted_runs()
    .expect("list")
    .iter()
    .map(Run::id)
    .collect();
  assert_eq!(interrupted, [lost.id(), crashed.id()]);

  for _ in 0..2 {
    for mut run in fixture.db.interrupted_runs().expect("list") {
      if run.continues_on_reconnect() {
        let events = run.continue_after_reconnect(at(30)).expect("continue");
        fixture.db.append_events(&run, &events).expect("append");
      }
    }
  }
  let lost = fixture.db.run(lost.id()).expect("load").expect("exists");
  assert_eq!(lost.status(), &RunStatus::Running);
  assert_eq!(
    lost.last_checkpoint().map(|c| c.revision().clone()),
    Some(revision('e'))
  );
  let continued = fixture
    .db
    .events(lost.id())
    .expect("events")
    .iter()
    .filter(|event| event.kind().name() == "continued")
    .count();
  assert_eq!(continued, 1);
  let interrupted: BTreeSet<_> = fixture
    .db
    .interrupted_runs()
    .expect("list")
    .iter()
    .map(Run::id)
    .collect();
  assert_eq!(interrupted, BTreeSet::from([crashed.id()]));
  assert_eq!(fixture.db.run(running.id()).expect("load"), Some(running));
}

#[test]
fn runs_running_lists_only_running_runs_in_number_order() {
  let mut fixture = Fixture::new();
  let first = fixture.implementing();
  let mut paused = fixture.implementing();
  let third = fixture.implementing();
  let _pending = fixture.create();
  fixture.apply(&mut paused, |run| run.request_pause(at(16)));
  fixture.apply(&mut paused, |run| run.pause(at(17)));
  let running: Vec<_> = fixture.db.running_runs().expect("list").iter().map(Run::id).collect();
  assert_eq!(running, [first.id(), third.id()]);
}

#[test]
fn runs_stopped_by_the_daily_budget_round_trip_and_resume() {
  let mut fixture = Fixture::new();
  let mut run = fixture.implementing();
  fixture.apply(&mut run, |run| run.stop_at_limit(RunLimit::DailyBudget, at(16)));
  let stored = fixture.db.run(run.id()).expect("load").expect("exists");
  assert_eq!(
    stored.status(),
    &RunStatus::Paused(PauseReason::LimitReached(RunLimit::DailyBudget))
  );
  // The daily budget is the project's: the run resumes as it is once the day has room.
  fixture.apply(&mut run, |run| run.resume(at(17)));
  assert_eq!(run.status(), &RunStatus::Running);
}

#[test]
fn runs_stopped_at_a_step_budget_round_trip_with_the_raised_budget() {
  let mut fixture = Fixture::new();
  let mut run = fixture.implementing();
  let implement = Fixture::exec(&run, "implement");
  let limit = RunLimit::StepBudget { execution: implement };
  fixture.apply(&mut run, |run| run.stop_at_limit(limit, at(16)));
  let stored = fixture.db.run(run.id()).expect("load").expect("exists");
  assert_eq!(stored.status(), &RunStatus::Paused(PauseReason::LimitReached(limit)));
  assert_eq!(stored, run);
  let budget = TokenBudget::new(600_000).expect("above zero");
  let max = TokenBudget::new(10_000_000).expect("above zero");
  fixture.apply(&mut run, |run| {
    run.raise_step_budget(budget, TokensUsed::new(400_000), max, at(17))
  });
  let stored = fixture.db.run(run.id()).expect("load").expect("exists");
  assert_eq!(stored.status(), &RunStatus::Running);
  assert_eq!(stored.step_budget(&key("implement")), Some(budget));
  assert_eq!(stored, run);
  let events = fixture.db.events(run.id()).expect("events");
  assert_eq!(Run::replay(&events), Ok(run));
}

#[test]
fn runs_changed_permissions_round_trip_while_pending_and_after_applying() {
  use orchester_core::{
    connection::{AgentProvider, invocation::Platform},
    permission::{AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess},
    run::{Checkpoint, PermissionsApply},
  };
  let mut fixture = Fixture::new();
  let mut run = fixture.implementing();
  let implement = Fixture::exec(&run, "implement");
  let profile = run.profile_of(&key("implement")).expect("agent step");
  let permissions = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::On,
  );
  let codex = (AgentProvider::CodexCli, Platform::MacOs);
  fixture.apply(&mut run, |run| {
    run.change_permissions(profile, permissions, PermissionsApply::Now, codex, at(16))
  });
  let stored = fixture.db.run(run.id()).expect("load").expect("exists");
  assert!(stored.restarts_for_permissions(implement));
  assert_eq!(stored, run);
  fixture.apply(&mut run, |run| run.record_checkpoint(implement, revision('e'), at(17)));
  let checkpoint = run.last_checkpoint().map(Checkpoint::id);
  fixture.apply(&mut run, |run| run.apply_permissions(implement, checkpoint, at(18)));
  let stored = fixture.db.run(run.id()).expect("load").expect("exists");
  assert_eq!(stored, run);
  let events = fixture.db.events(run.id()).expect("events");
  assert_eq!(Run::replay(&events), Ok(run));
}

/// Two runners' worth of writers (each run's task has its own connection) record transitions of
/// their runs at the same time: every write waits for the other instead of failing as locked.
#[test]
fn runs_concurrent_writers_on_one_project_wait_for_each_other() {
  let mut fixture = Fixture::new();
  let runs = [fixture.implementing(), fixture.implementing()];
  let root = fixture.repo.path().to_path_buf();
  let writers: Vec<_> = runs
    .into_iter()
    .map(|mut run| {
      let root = root.clone();
      std::thread::spawn(move || {
        let mut db = ProjectDb::open(&root).expect("open");
        for tick in 0..200 {
          let events = run
            .record_usage(TokensUsed::new(1), at(100 + tick))
            .expect("usage is recorded");
          db.append_events(&run, &events)
            .expect("the write waits for the other writer");
        }
      })
    })
    .collect();
  for writer in writers {
    writer.join().expect("the writer finishes");
  }
}
