use std::{
  collections::{BTreeMap, BTreeSet},
  num::NonZeroU32,
};

use orchester_core::{
  id::{ConnectionId, ProjectId, ResourceId, RunId, StepExecutionId, TaskId, VersionId},
  name::Name,
  permission::StepPermissions,
  project::{Project, ProjectKey, TaskKey},
  resource::{
    AgentProfile, FailureRoute, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Step, StepKey,
    StepKind, Version, VersionNumber,
  },
  revision::Revision,
  run::{BranchName, NewRun, Plan, PlanDecision, PublishedVersions, Run, RunEvent, RunNumber, RunSnapshot, StepStatus},
  task::{Task, TaskDetails},
  time::Timestamp,
  tokens::TokenBudget,
};

pub fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

pub fn key(value: &str) -> StepKey {
  StepKey::new(value).expect("valid key")
}

pub fn model(value: &str) -> ModelId {
  ModelId::new(value).expect("valid model")
}

pub fn revision(hex_digit: char) -> Revision {
  Revision::new(&hex_digit.to_string().repeat(40)).expect("valid revision")
}

pub fn tokens(count: u64) -> TokenBudget {
  TokenBudget::new(count).expect("a budget above zero")
}

pub fn task_key(number: u32) -> TaskKey {
  TaskKey::new(
    ProjectKey::new("ORC").expect("valid key"),
    NonZeroU32::new(number).expect("positive"),
  )
}

/// Draft task ORC-12, "Add GitHub sign-in".
pub fn task(brief: &str, criteria: &[&str]) -> Task {
  let details = TaskDetails {
    title: Name::new("Add GitHub sign-in").expect("valid title"),
    brief: brief.into(),
    acceptance_criteria: criteria.iter().map(|criterion| (*criterion).to_owned()).collect(),
    pipeline: None,
    limits: None,
  };
  Task::draft(TaskId::generate(), task_key(12), details, at(1))
}

pub fn project() -> Project {
  Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("valid key"),
    Name::new("Kontur").expect("valid name"),
  )
}

pub fn version<B>(number: u32, body: B) -> Version<B> {
  let number = VersionNumber::new(NonZeroU32::new(number).expect("positive version"));
  Version::new(VersionId::generate(), number, body, at(1))
}

pub fn profile(role: &str, model_id: &str) -> AgentProfile {
  AgentProfile::new(
    Name::new(role).expect("valid role"),
    String::new(),
    PromptText::new("Work.").expect("valid prompt"),
    ConnectionId::generate(),
    model(model_id),
    ProviderSettings::default(),
  )
}

pub fn agent_step(value: &str, profile: ResourceId, depends_on: &[&str]) -> Step {
  Step {
    key: key(value),
    name: Name::new(value).expect("valid name"),
    kind: StepKind::Agent {
      profile: Some(profile),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("valid prompt"),
    depends_on: depends_on
      .iter()
      .map(|dependency| key(dependency))
      .collect::<BTreeSet<_>>(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }
}

/// Published configuration of the sample pipeline: research → plan (gated) → implement →
/// checks ∥ review → pr. A failed check returns to implement at most twice.
pub struct Config {
  pub pipeline: ResourceId,
  pub pipeline_version: Version<Pipeline>,
  pub builder: ResourceId,
  pub profiles: BTreeMap<ResourceId, Version<AgentProfile>>,
}

impl Config {
  pub fn feature_delivery() -> Self {
    let builder = ResourceId::generate();
    let reviewer = ResourceId::generate();
    let plan = Step {
      approval_gate: true,
      ..agent_step("plan", builder, &["research"])
    };
    let checks = Step {
      on_failure: Some(FailureRoute {
        return_to: key("implement"),
        max_retries: NonZeroU32::new(2).expect("positive"),
      }),
      ..agent_step("checks", builder, &["implement"])
    };
    let steps = vec![
      agent_step("research", builder, &[]),
      plan,
      agent_step("implement", builder, &["plan"]),
      checks,
      agent_step("review", reviewer, &["implement"]),
      agent_step("pr", builder, &["checks", "review"]),
    ];
    let pipeline = Pipeline::new(steps).expect("valid pipeline");
    Self {
      pipeline: ResourceId::generate(),
      pipeline_version: version(3, pipeline),
      builder,
      profiles: BTreeMap::from([
        (builder, version(2, profile("Builder", "model-b"))),
        (reviewer, version(1, profile("Reviewer", "model-r"))),
      ]),
    }
  }

  pub fn snapshot(&self) -> RunSnapshot {
    let empty_tools = BTreeMap::new();
    let empty_instructions = BTreeMap::new();
    let published = PublishedVersions {
      profiles: &self.profiles,
      tools: &empty_tools,
      instructions: &empty_instructions,
    };
    RunSnapshot::build(self.pipeline, &self.pipeline_version, published).expect("every reference is published")
  }
}

pub fn new_run(snapshot: RunSnapshot) -> NewRun {
  NewRun {
    task: TaskId::generate(),
    snapshot,
    branch: BranchName::new("orchester/orc-129").expect("valid branch"),
    base: revision('a'),
    budget: tokens(1_400_000),
    max_duration: std::time::Duration::from_secs(30 * 60),
  }
}

/// A created run with its log so far.
pub struct Logged {
  pub run: Run,
  pub log: Vec<RunEvent>,
}

impl Logged {
  pub fn pending() -> Self {
    let number = RunNumber::new(NonZeroU32::new(128).expect("positive"));
    let (run, created) = Run::create(
      RunId::generate(),
      number,
      new_run(Config::feature_delivery().snapshot()),
      at(10),
    )
    .expect("positive time limit");
    Self {
      run,
      log: vec![created],
    }
  }

  pub fn started() -> Self {
    let mut logged = Self::pending();
    logged.apply(|run| run.start(at(11)));
    logged
  }

  /// Runs `transition`, which must be accepted, and appends its events.
  pub fn apply(&mut self, transition: impl FnOnce(&mut Run) -> Result<Vec<RunEvent>, orchester_core::run::RunError>) {
    let events = transition(&mut self.run).expect("transition is accepted");
    self.log.extend(events);
  }

  /// The current execution of `step`.
  pub fn exec(&self, step: &str) -> StepExecutionId {
    self
      .run
      .latest_execution(&key(step))
      .expect("step has an execution")
      .id()
  }

  /// Starts and passes `step`, which must not be gated.
  pub fn pass(&mut self, step: &str) {
    let execution = self.exec(step);
    self.apply(|run| run.start_step(execution, at(20)));
    self.apply(|run| run.complete_step(execution, at(21)));
  }

  /// Starts `step`, which must be gated, and proposes `text` as its next plan after `earlier`.
  pub fn propose(&mut self, step: &str, text: &str, earlier: &[Plan]) -> Plan {
    let execution = self.exec(step);
    if self.run.execution(execution).map(|execution| execution.status()) == Some(StepStatus::Pending) {
      self.apply(|run| run.start_step(execution, at(22)));
    }
    let (events, plan) = self
      .run
      .propose_plan(execution, text.to_owned(), earlier, at(23))
      .expect("the plan is proposed");
    self.log.extend(events);
    plan
  }

  /// Starts, completes and approves the gated `plan` step after `research`.
  pub fn through_plan(&mut self) {
    self.pass("research");
    let mut plan = self.propose("plan", "1. Add the provider.", &[]);
    self.apply(|run| run.decide_plan(&mut plan, PlanDecision::Approved, at(24)));
  }
}
