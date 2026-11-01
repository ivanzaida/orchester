//! A project with tasks and runs in a temporary repository, for tests that read what the
//! project database holds.

#![allow(dead_code, reason = "shared by several test binaries, each of which uses a part")]

use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  time::Duration,
};

use orchester_core::{
  id::{ConnectionId, IdempotencyKey, ProjectId, ResourceId, TaskId},
  memory::{
    AgentProvenance, AgentWrite, Confidence, Memory, MemoryContent, MemoryKind, MemoryScope, MemorySettings,
    SecretScreen,
  },
  name::Name,
  permission::StepPermissions,
  project::{Project, ProjectKey},
  resource::{
    AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step, StepKey, StepKind,
  },
  revision::Revision,
  run::{Answerer, BranchName, NewRun, Plan, PlanDecision, PublishedVersions, Run, RunEvent, RunSnapshot},
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
    Self::with_gate(false, "ORC")
  }

  /// The repository, for a test that opens its project another way.
  pub fn into_repo(self) -> TempDir {
    self.repo
  }

  /// [`Fixture::new`] whose project has the key `key`.
  pub fn keyed(key: &str) -> Self {
    Self::with_gate(false, key)
  }

  /// [`Fixture::new`] whose step, Implement, has an approval gate.
  pub fn gated() -> Self {
    Self::with_gate(true, "ORC")
  }

  /// A two-agent pipeline whose Reviewer runs after Builder.
  pub fn reviewed() -> Self {
    let mut fixture = Self::new();
    let implement = fixture.snapshot.body().steps()[0].clone();
    let StepKind::Agent {
      profile: Some(builder), ..
    } = &implement.kind
    else {
      panic!("the fixture's Implement step has a Builder profile");
    };
    let profile = AgentProfile::new(
      name("Reviewer"),
      String::new(),
      PromptText::new("Review.").expect("prompt"),
      ConnectionId::generate(),
      ModelId::new("gpt-5.5").expect("valid model"),
      ProviderSettings::default(),
    );
    let reviewer = fixture
      .db
      .create_resource(Slug::new("reviewer").expect("slug"), name("Reviewer"), &profile, at(6))
      .expect("profile")
      .id();
    let profiles = BTreeMap::from([
      (
        *builder,
        fixture
          .db
          .latest_version::<AgentProfile>(*builder)
          .expect("read Builder")
          .expect("Builder is published"),
      ),
      (
        reviewer,
        fixture
          .db
          .publish_draft::<AgentProfile>(reviewer, at(7))
          .expect("publish Reviewer"),
      ),
    ]);
    let review = Step {
      key: StepKey::new("review").expect("key"),
      name: name("Review"),
      kind: StepKind::Agent {
        profile: Some(reviewer),
        repository: RepositoryAccess::ReadOnly,
        model: None,
        permissions: StepPermissions::default(),
      },
      instructions: PromptText::new("").expect("prompt"),
      depends_on: BTreeSet::from([implement.key.clone()]),
      approval_gate: false,
      on_failure: None,
      budget: None,
    };
    let pipeline = Pipeline::new(vec![implement, review]).expect("pipeline");
    let delivery = fixture
      .db
      .create_resource(
        Slug::new("review-delivery").expect("slug"),
        name("Review delivery"),
        &pipeline,
        at(8),
      )
      .expect("pipeline")
      .id();
    let version = fixture.db.publish_draft::<Pipeline>(delivery, at(9)).expect("publish");
    let (tools, instructions) = (BTreeMap::new(), BTreeMap::new());
    fixture.snapshot = RunSnapshot::build(
      delivery,
      &version,
      PublishedVersions {
        profiles: &profiles,
        tools: &tools,
        instructions: &instructions,
        skills: &BTreeMap::new(),
      },
    )
    .expect("snapshot");
    fixture
  }

  /// [`Fixture::gated`] whose pipeline runs Builder's steps Research and Docs beside its gated Implement, so
  /// several steps of one run can wait at once.
  pub fn parallel() -> Self {
    let mut fixture = Self::gated();
    let implement = fixture.snapshot.body().steps()[0].clone();
    let StepKind::Agent {
      profile: Some(builder), ..
    } = &implement.kind
    else {
      panic!("the fixture's Implement step has a Builder profile");
    };
    let profiles = BTreeMap::from([(
      *builder,
      fixture
        .db
        .latest_version::<AgentProfile>(*builder)
        .expect("read Builder")
        .expect("Builder is published"),
    )]);
    let beside = |key: &str, title: &str| Step {
      key: StepKey::new(key).expect("key"),
      name: name(title),
      approval_gate: false,
      ..implement.clone()
    };
    let pipeline = Pipeline::new(vec![
      implement.clone(),
      beside("research", "Research"),
      beside("docs", "Docs"),
    ])
    .expect("pipeline");
    let delivery = fixture
      .db
      .create_resource(
        Slug::new("parallel-delivery").expect("slug"),
        name("Parallel delivery"),
        &pipeline,
        at(8),
      )
      .expect("pipeline")
      .id();
    let version = fixture.db.publish_draft::<Pipeline>(delivery, at(9)).expect("publish");
    let (tools, instructions) = (BTreeMap::new(), BTreeMap::new());
    fixture.snapshot = RunSnapshot::build(
      delivery,
      &version,
      PublishedVersions {
        profiles: &profiles,
        tools: &tools,
        instructions: &instructions,
        skills: &BTreeMap::new(),
      },
    )
    .expect("snapshot");
    fixture
  }

  fn with_gate(approval_gate: bool, key: &str) -> Self {
    let repo = tempfile::tempdir().expect("temporary directory");
    fs::create_dir(repo.path().join(".git")).expect("create .git");
    let project = Project::new(
      ProjectId::generate(),
      ProjectKey::new(key).expect("key"),
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
      skills: &std::collections::BTreeMap::new(),
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
      limits: TaskLimits::new(Some(budget(1_000_000)), Some(Duration::from_secs(1_800))).expect("limits"),
    };
    self
      .db
      .create_tasks(IdempotencyKey::generate(), vec![details], now)
      .expect("task")[0]
      .id()
  }

  /// A complete task titled `title`, created at `now` and queued with the fixture's pipeline, as
  /// Run queues it before its first run.
  pub fn queued_task(&mut self, title: &str, now: Timestamp) -> TaskId {
    let pinned = self.snapshot.pipeline();
    let details = TaskDetails {
      title: name(title),
      brief: "Sign in with a password.".to_owned(),
      acceptance_criteria: vec!["A person can sign in".to_owned()],
      pipeline: Some(pinned.resource),
      limits: TaskLimits::new(Some(budget(1_000_000)), Some(Duration::from_secs(1_800))).expect("limits"),
    };
    let mut task = self
      .db
      .create_tasks(IdempotencyKey::generate(), vec![details], now)
      .expect("task")
      .remove(0);
    task.queue(pinned.number).expect("a complete draft queues");
    self
      .db
      .save_task(&task)
      .expect("save")
      .saved()
      .expect("no conflict")
      .id()
  }

  /// Keeps the connection Builder's profile names, as an agent on `provider`.
  pub fn connect_builder(&mut self, provider: orchester_core::connection::AgentProvider) {
    let StepKind::Agent {
      profile: Some(builder), ..
    } = self.snapshot.body().steps()[0].kind
    else {
      panic!("the fixture's first step has a Builder profile");
    };
    let profile = self
      .db
      .latest_version::<AgentProfile>(builder)
      .expect("read Builder")
      .expect("Builder is published");
    let connection = orchester_core::connection::AgentConnection::new(
      profile.body().connection(),
      name("Agent"),
      provider,
      self.repo.path().join("bin").join("agent.exe"),
      at(1),
    )
    .expect("absolute executable");
    orchester_core::store::ConnectionStore::create_connection(&mut self.db, &connection).expect("kept");
  }

  /// A run of `task`, created at `now` and not started.
  pub fn pending_run(&mut self, task: TaskId, now: Timestamp) -> Run {
    self.pending_run_into(task, (&"a".repeat(40), None), now)
  }

  /// A run of `task` started at `now` at the commit `base` of the checkout's branch `base_branch`, which
  /// merging it brings its work into.
  pub fn started_run_from(&mut self, task: TaskId, (base, base_branch): (&str, &str), now: Timestamp) -> Run {
    let into = BranchName::new(base_branch).expect("branch");
    let run = self.pending_run_into(task, (base, Some(into)), now);
    self.start(run, now)
  }

  fn pending_run_into(&mut self, task: TaskId, (base, base_branch): (&str, Option<BranchName>), now: Timestamp) -> Run {
    self.runs += 1;
    let new = NewRun {
      task,
      snapshot: self.snapshot.clone(),
      branch: BranchName::new(&format!("orchester/orc-run-{}", self.runs)).expect("branch"),
      base: Revision::new(base).expect("revision"),
      base_branch,
      base_source: orchester_core::integration::base::RunBaseSource::CheckedOut,
      budget: Some(budget(1_000_000)),
      max_duration: Some(Duration::from_secs(1_800)),
    };
    self.db.create_run(IdempotencyKey::generate(), new, now).expect("run")
  }

  /// A run of `task`, started at `now`: running its one step.
  pub fn started_run(&mut self, task: TaskId, now: Timestamp) -> Run {
    let run = self.pending_run(task, now);
    self.start(run, now)
  }

  fn start(&mut self, mut run: Run, now: Timestamp) -> Run {
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

  /// Records `by`'s `decision` on `plan` of `run` at `now`.
  pub fn decide(&mut self, run: &mut Run, plan: &mut Plan, (decision, by): (PlanDecision, Answerer), now: Timestamp) {
    let events = run.decide_plan(plan, decision, by, now).expect("decided");
    self.db.append_events_with_plan(run, &events, plan).expect("append");
  }

  /// Stores `events`, which `run` just recorded.
  pub fn record(&mut self, run: &Run, events: &[RunEvent]) {
    self.db.append_events(run, events).expect("append");
  }
}

/// A record `run`'s agent writes to the project's memory at `now`, which waits for approval.
pub fn proposed(run: &Run, text: &str, now: i64) -> Memory {
  let write = AgentWrite {
    provenance: AgentProvenance {
      profile: ResourceId::generate(),
      run: run.id(),
      step: StepKey::new("implement").expect("key"),
      execution: run.current_attempt().expect("attempt").executions()[0].id(),
    },
    confidence: Confidence::Medium,
    sources: Vec::new(),
  };
  let content = MemoryContent::new(MemoryKind::Fact, text).expect("content");
  let screened = SecretScreen::new(Vec::new()).check(content).expect("no secrets");
  Memory::by_agent(
    MemoryScope::Project,
    write,
    screened,
    &MemorySettings::default(),
    at(now),
  )
}
