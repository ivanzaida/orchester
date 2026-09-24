use std::{
  collections::{BTreeMap, BTreeSet},
  num::NonZeroU32,
};

use orchester_core::{
  id::{ConnectionId, ResourceId, RunId, StepExecutionId, TaskId, VersionId},
  money::{Currency, Money},
  name::Name,
  resource::{
    AgentProfile, FailureRoute, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Step, StepKey,
    StepKind, Version, VersionNumber,
  },
  revision::Revision,
  run::{BranchName, NewRun, PublishedVersions, Run, RunEvent, RunNumber, RunSnapshot},
  time::Timestamp,
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

pub fn usd(micros: i64) -> Money {
  Money::from_micros(micros, Currency::USD)
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
      profile,
      repository: RepositoryAccess::Write,
      model: None,
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
    budget: usd(5_000_000),
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
    .expect("positive budget");
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

  /// Starts, completes and approves the gated `plan` step after `research`.
  pub fn through_plan(&mut self) {
    self.pass("research");
    let plan = self.exec("plan");
    self.apply(|run| run.start_step(plan, at(22)));
    self.apply(|run| run.complete_step(plan, at(23)));
    self.apply(|run| run.decide_plan(plan, orchester_core::run::PlanDecision::Approved, at(24)));
  }
}
