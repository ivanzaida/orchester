//! A project with tasks and runs in a temporary repository, for tests that read what the
//! project database holds.

#![allow(dead_code, reason = "shared by several test binaries, each of which uses a part")]

use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  time::Duration,
};

use orchester_core::{
  connection::mcp::McpServerSelection,
  id::{ChatId, ConnectionId, IdempotencyKey, ProjectId, ResourceId, TaskId},
  memory::{
    AgentProvenance, AgentWrite, Confidence, Memory, MemoryContent, MemoryKind, MemoryScope, MemorySettings,
    SecretScreen,
  },
  name::Name,
  orchestrator::{Chat, Subscription, WatchTarget},
  permission::StepPermissions,
  project::{Project, ProjectKey},
  resource::{
    AgentProfile, Draft, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Resource, ResourceKind,
    Slug, Step, StepKey, StepKind, VersionNumber,
  },
  revision::Revision,
  run::{Answerer, BranchName, NewRun, Plan, PlanDecision, PublishedVersions, Run, RunEvent, RunSnapshot},
  sharing::TaskRevision,
  store::{ResourceStore, RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_store::{CachedVersion, ProjectDb};
use tempfile::TempDir;

mod team_profile;
use team_profile::{keep_as_team_profile, team_server_references};

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
    let (delivery, version) = fixture
      .db
      .published_pipeline_for_test("review-delivery", "Review delivery", &pipeline, at(8))
      .expect("pipeline");
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
    let (delivery, version) = fixture
      .db
      .published_pipeline_for_test("parallel-delivery", "Parallel delivery", &pipeline, at(8))
      .expect("pipeline");
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

  /// [`Fixture::new`] whose Builder profile is the team's, its cached version naming the team MCP servers `servers`
  /// by `TeamResource` references.
  pub fn choosing(servers: &[ResourceId]) -> Self {
    Self::with_setup(false, "ORC", Some(servers))
  }

  fn with_gate(approval_gate: bool, key: &str) -> Self {
    Self::with_setup(approval_gate, key, None)
  }

  fn with_setup(approval_gate: bool, key: &str, team_servers: Option<&[ResourceId]>) -> Self {
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
    let published = db.publish_draft::<AgentProfile>(builder, at(3)).expect("publish");
    if let Some(servers) = team_servers {
      keep_as_team_profile(&mut db, builder, &published, servers);
    }
    let profiles = BTreeMap::from([(builder, published)]);
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
    let (delivery, version) = db
      .published_pipeline_for_test("delivery", "Delivery", &pipeline, at(4))
      .expect("pipeline");
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

  /// Builder's next version chooses the local MCP server `server`, and the runs the fixture starts from now on pin it.
  pub fn choose_local_server(&mut self, server: ConnectionId) {
    let StepKind::Agent {
      profile: Some(builder), ..
    } = self.snapshot.body().steps()[0].kind.clone()
    else {
      panic!("the fixture's first step has a Builder profile");
    };
    let latest = self
      .db
      .latest_version::<AgentProfile>(builder)
      .expect("read")
      .expect("published");
    let body = latest
      .body()
      .clone()
      .with_mcp_servers(McpServerSelection::new(vec![server]).expect("one server"));
    self
      .db
      .save_draft(builder, &Draft::new(body, Some(latest.number()), at(6)))
      .expect("draft");
    let next = self.db.publish_draft::<AgentProfile>(builder, at(7)).expect("publish");
    let delivery = self.db.resources(ResourceKind::Pipeline).expect("pipelines")[0].id();
    let pipeline = self
      .db
      .latest_version::<Pipeline>(delivery)
      .expect("read")
      .expect("published");
    let (tools, instructions) = (BTreeMap::new(), BTreeMap::new());
    let profiles = BTreeMap::from([(builder, next)]);
    let published = PublishedVersions {
      profiles: &profiles,
      tools: &tools,
      instructions: &instructions,
      skills: &BTreeMap::new(),
    };
    self.snapshot = RunSnapshot::build(delivery, &pipeline, published).expect("snapshot");
  }

  /// The team MCP server `id` named `called`, kept in this project as a synced one is.
  pub fn keep_team_server(&mut self, id: ResourceId, called: &str) {
    let server = Resource::new(
      id,
      ResourceKind::McpServer,
      Slug::new(&called.to_lowercase()).expect("slug"),
      name(called),
      at(1),
    );
    let revision = TaskRevision::try_from(1_u64).expect("a revision");
    self
      .db
      .keep_shared_resource(&server, revision)
      .expect("the team MCP server is kept");
  }

  /// The team's next version of the Builder profile, cached as the server confirmed it and naming `servers` by
  /// `TeamResource` references; the fixture's run keeps the version it pinned.
  pub fn cache_next_builder_version(&mut self, servers: &[ResourceId]) {
    let step = self.snapshot.body().steps()[0].clone();
    let StepKind::Agent {
      profile: Some(builder), ..
    } = &step.kind
    else {
      panic!("the fixture's first step has a Builder profile");
    };
    let pinned = self
      .db
      .cached_version::<AgentProfile>(*builder, VersionNumber::FIRST)
      .expect("read")
      .expect("the pinned version is cached");
    let next = CachedVersion {
      number: VersionNumber::FIRST.next().expect("a next version"),
      references: team_server_references(servers),
      ..pinned
    };
    self
      .db
      .cache_shared_version(*builder, &next)
      .expect("the next team version is cached");
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

  /// What a run of `task` starts from, as [`Fixture::pending_run`] makes it, for a test that creates runs itself.
  pub fn new_run_for(&self, task: TaskId) -> NewRun {
    NewRun {
      task,
      snapshot: self.snapshot.clone(),
      branch: BranchName::new("orchester/orc-run-0").expect("branch"),
      base: Revision::new(&"a".repeat(40)).expect("revision"),
      base_branch: None,
      base_source: orchester_core::integration::base::RunBaseSource::CheckedOut,
      budget: Some(budget(1_000_000)),
      max_duration: Some(Duration::from_secs(1_800)),
    }
  }

  /// A run of `task`, created at `now` and not started.
  pub fn pending_run(&mut self, task: TaskId, now: Timestamp) -> Run {
    self.pending_run_into(task, (&"a".repeat(40), None), (IdempotencyKey::generate(), now))
  }

  /// A run of `task` started at `now` at the commit `base` of the checkout's branch `base_branch`, which
  /// merging it brings its work into.
  pub fn started_run_from(&mut self, task: TaskId, (base, base_branch): (&str, &str), now: Timestamp) -> Run {
    let into = BranchName::new(base_branch).expect("branch");
    let run = self.pending_run_into(task, (base, Some(into)), (IdempotencyKey::generate(), now));
    self.start(run, now)
  }

  /// The first run of `task`, recorded with the task's first-run key as a reviewed group's first run is, started at
  /// `now`.
  pub fn first_run(&mut self, task: TaskId, now: Timestamp) -> Run {
    let run = self.pending_run_into(task, (&"a".repeat(40), None), (task.first_run_key(), now));
    self.start(run, now)
  }

  fn pending_run_into(
    &mut self,
    task: TaskId,
    (base, base_branch): (&str, Option<BranchName>),
    (key, now): (IdempotencyKey, Timestamp),
  ) -> Run {
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
    self.db.create_run(key, new, now).expect("run")
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

  /// `chat`, kept with it when it is new, watches everything in the project from `since` on: the events of every
  /// task no more specific subscription leads go to it, and only subscriptions route events (ORC-895).
  pub fn watch_everything(&mut self, chat: &Chat, since: Timestamp) {
    self.db.keep_watch_entries(chat, &[], since).expect("conversation");
    let subscription = Subscription::new(chat.id(), WatchTarget::Project, "", since);
    self.db.start_subscription(&subscription).expect("watching");
  }

  /// A new conversation titled "Everything", started at `since`, that watches everything in the project.
  pub fn everything(&mut self, since: Timestamp) -> Chat {
    let chat = Chat::named(ChatId::generate(), "Everything", since);
    self.watch_everything(&chat, since);
    chat
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
