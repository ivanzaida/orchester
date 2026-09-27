//! A project with tasks and runs in a temporary repository, for tests that read what the
//! project database holds.

#![allow(dead_code, reason = "shared by several test binaries, each of which uses a part")]

use std::{collections::BTreeMap, fs, time::Duration};

use orchester_core::{
  id::{ConnectionId, IdempotencyKey, ProjectId, TaskId},
  name::Name,
  permission::StepPermissions,
  project::{Project, ProjectKey},
  resource::{
    AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step, StepKey, StepKind,
  },
  revision::Revision,
  run::{BranchName, NewRun, Plan, PlanDecision, PublishedVersions, Run, RunEvent, RunSnapshot},
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

fn name(value: &str) -> Name {
  Name::new(value).expect("valid name")
}

fn budget(tokens: u64) -> TokenBudget {
  TokenBudget::new(tokens).expect("above zero")
}

/// A project whose one-step pipeline "Delivery" is published, so tasks can run it; its step waits
/// for a plan decision in [`Fixture::gated`].
pub struct Fixture {
  pub repo: TempDir,
  pub db: ProjectDb,
  snapshot: RunSnapshot,
  runs: usize,
}

impl Fixture {
  pub fn new() -> Self {
    Self::with_gate(false)
  }

  /// [`Fixture::new`] whose step, Implement, has an approval gate.
  pub fn gated() -> Self {
    Self::with_gate(true)
  }

  fn with_gate(approval_gate: bool) -> Self {
    let repo = tempfile::tempdir().expect("temporary directory");
    fs::create_dir(repo.path().join(".git")).expect("create .git");
    let project = Project::new(
      ProjectId::generate(),
      ProjectKey::new("ORC").expect("key"),
      name("Kontur"),
    );
    let mut db = ProjectDb::create(repo.path(), project).expect("create project");
    let profile = AgentProfile::new(
      name("Builder"),
      String::new(),
      PromptText::new("Build.").expect("prompt"),
      ConnectionId::generate(),
      ModelId::new("gpt-5.5").expect("valid model"),
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
    };
    let snapshot = RunSnapshot::build(delivery, &version, published).expect("snapshot");
    Self {
      repo,
      db,
      snapshot,
      runs: 0,
    }
  }

  /// A task titled `title`, created at `now`.
  pub fn task(&mut self, title: &str, now: Timestamp) -> TaskId {
    let details = TaskDetails {
      title: name(title),
      brief: String::new(),
      acceptance_criteria: Vec::new(),
      pipeline: None,
      limits: Some(TaskLimits::new(budget(1_000_000), Duration::from_secs(1_800)).expect("limits")),
    };
    self
      .db
      .create_tasks(IdempotencyKey::generate(), vec![details], now)
      .expect("task")[0]
      .id()
  }

  /// A run of `task`, started at `now`: running its one step.
  pub fn started_run(&mut self, task: TaskId, now: Timestamp) -> Run {
    self.runs += 1;
    let new = NewRun {
      task,
      snapshot: self.snapshot.clone(),
      branch: BranchName::new(&format!("orchester/orc-run-{}", self.runs)).expect("branch"),
      base: Revision::new(&"a".repeat(40)).expect("revision"),
      base_branch: None,
      budget: budget(1_000_000),
      max_duration: Duration::from_secs(1_800),
    };
    let mut run = self.db.create_run(IdempotencyKey::generate(), new, now).expect("run");
    let events = run.start(now).expect("start");
    self.record(&run, &events);
    let execution = run.current_attempt().expect("attempt").executions()[0].id();
    let events = run.start_step(execution, now).expect("start step");
    self.record(&run, &events);
    run
  }

  /// `run` passes its one step at `now`, so every step passed.
  pub fn succeed(&mut self, run: &mut Run, now: Timestamp) {
    let execution = run.current_attempt().expect("attempt").executions()[0].id();
    let events = run.complete_step(execution, now).expect("complete step");
    self.record(run, &events);
  }

  /// The gated step of `run` proposes `text` as its next plan at `now`.
  pub fn propose(&mut self, run: &mut Run, text: &str, now: Timestamp) -> Plan {
    let execution = run.current_attempt().expect("attempt").executions()[0].id();
    let earlier = self.db.plans(run.id()).expect("plans");
    let (events, plan) = run
      .propose_plan(execution, text.to_owned(), &earlier, now)
      .expect("proposed");
    self.db.append_events_with_plan(run, &events, &plan).expect("append");
    plan
  }

  /// Records `decision` on `plan` of `run` at `now`.
  pub fn decide(&mut self, run: &mut Run, plan: &mut Plan, decision: PlanDecision, now: Timestamp) {
    let events = run.decide_plan(plan, decision, now).expect("decided");
    self.db.append_events_with_plan(run, &events, plan).expect("append");
  }

  /// Stores `events`, which `run` just recorded.
  pub fn record(&mut self, run: &Run, events: &[RunEvent]) {
    self.db.append_events(run, events).expect("append");
  }
}
