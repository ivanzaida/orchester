//! What the QA project's tasks run on: the Codex connection, the design's agent profiles (Explorer, Planner,
//! Builder, Reviewer) and the pipelines, published as people publish them.

use std::{collections::BTreeMap, path::Path};

use orchester_core::{
  connection::{AgentConnection, AgentProvider, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn},
  id::{ConnectionId, ResourceId},
  permission::{AgentPermissions, CommandPermission, NetworkAccess, StepPermissions},
  resource::{
    AgentProfile, CommandAction, CommandArg, CommandProgram, Draft, FailureRoute, ModelId, Pipeline, PromptText,
    ProviderSettings, RepositoryAccess, ResourceBody, Slug, StarterAssignments, StarterStep, StarterStepText, Step,
    StepKey, StepKind, Tool, Version, feature_delivery,
  },
  run::{PublishedVersions, RunSnapshot},
  store::{ConnectionStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{SeedError, name};

/// The version `orchester-fake-codex` stands in for, as the runner's tests record its check.
const FAKE_CODEX_VERSION: &str = "0.139.0";
/// The model of every profile.
pub const MODEL: &str = "gpt-5.5";
/// The command lines of the static catalogue's tools, by the key of the step that runs them.
const CHECKS: &[&str] = &["cargo", "test"];
const PULL_REQUEST: &[&str] = &["gh", "pr", "create", "--fill"];

/// A pipeline's latest published version, pinned the way a run pins it.
pub struct Published {
  snapshot: RunSnapshot,
}

impl Published {
  pub fn id(&self) -> ResourceId {
    self.snapshot.pipeline().resource
  }

  pub fn snapshot(&self) -> &RunSnapshot {
    &self.snapshot
  }
}

/// The pipelines tasks choose from.
pub struct Catalogue {
  pub feature_delivery: Published,
  pub quick_fix: Published,
}

/// The project's Codex connection, `Codex · Kontur`. With `fake_codex` it runs that program, checked and
/// signed in as the runner's tests record it (the fake answers no `--version`); without, no connection is
/// stored and the profiles name one that is missing, so nothing can run, as with the old app's test
/// fixture.
pub fn codex(db: &mut ProjectDb, fake_codex: Option<&Path>, now: Timestamp) -> Result<ConnectionId, SeedError> {
  let Some(program) = fake_codex else {
    return Ok(ConnectionId::generate());
  };
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    name("Codex · Kontur"),
    AgentProvider::CodexCli,
    program.to_path_buf(),
    now,
  )?;
  connection.record_check(ConnectionCheck {
    at: now,
    result: CheckResult::Available {
      version: FAKE_CODEX_VERSION.to_owned(),
      sign_in: SignIn::SignedIn,
      catalog: ModelCatalog::new(CatalogSource::Provider, Vec::new()),
    },
  })?;
  let connection = connection.into_connection();
  db.create_connection(&connection)?;
  Ok(connection.id())
}

/// The design's agent profiles, published on `connection`.
struct Profiles {
  explorer: ResourceId,
  planner: ResourceId,
  builder: ResourceId,
  reviewer: ResourceId,
  versions: BTreeMap<ResourceId, Version<AgentProfile>>,
}

impl Profiles {
  fn publish(db: &mut ProjectDb, connection: ConnectionId, now: Timestamp) -> Result<Self, SeedError> {
    Self::publish_on(db, |_| (connection, MODEL), now)
  }

  /// The profiles, each on the connection and model `on` gives for its role.
  fn publish_on(
    db: &mut ProjectDb,
    on: impl Fn(&str) -> (ConnectionId, &'static str),
    now: Timestamp,
  ) -> Result<Self, SeedError> {
    let mut versions = BTreeMap::new();
    let mut profile = |role: &str, duties: &str, access: RepositoryAccess| -> Result<ResourceId, SeedError> {
      let (connection, model) = on(role);
      let body = AgentProfile::new(
        name(role),
        duties.to_owned(),
        PromptText::new(duties).expect("the profiles' instructions are valid prompts"),
        connection,
        ModelId::new(model).expect("a valid model id"),
        ProviderSettings::default(),
      )
      .with_permissions(AgentPermissions::new(
        access,
        CommandPermission::AnyInWorktree,
        NetworkAccess::Off,
      ));
      let (id, version) = publish(db, role, vec![body], now)?;
      versions.insert(id, version);
      Ok(id)
    };
    let explorer = profile(
      "Explorer",
      "Reads the code and reports what a task touches.",
      RepositoryAccess::ReadOnly,
    )?;
    let planner = profile(
      "Planner",
      "Turns the research into a plan a person approves.",
      RepositoryAccess::ReadOnly,
    )?;
    let builder = profile(
      "Builder",
      "Implements the approved plan in small commits.",
      RepositoryAccess::Write,
    )?;
    let reviewer = profile(
      "Reviewer",
      "Reviews the change against the acceptance criteria.",
      RepositoryAccess::ReadOnly,
    )?;
    Ok(Self {
      explorer,
      planner,
      builder,
      reviewer,
      versions,
    })
  }
}

/// Creates `name`'s resource with the first of `bodies` and publishes each in turn: version N is the Nth.
fn publish<B: ResourceBody>(
  db: &mut ProjectDb,
  name_text: &str,
  bodies: Vec<B>,
  now: Timestamp,
) -> Result<(ResourceId, Version<B>), SeedError> {
  let slug = Slug::from_name(name_text).expect("the catalogue's names have letters");
  let mut bodies = bodies.into_iter();
  let first = bodies.next().expect("every resource has a first version");
  let id = db.create_resource(slug, name(name_text), &first, now)?.id();
  let mut version = db.publish_draft::<B>(id, now)?;
  for body in bodies {
    db.save_draft(id, &Draft::new(body, Some(version.number()), now))?;
    version = db.publish_draft::<B>(id, now)?;
  }
  Ok((id, version))
}

fn command_tool(
  db: &mut ProjectDb,
  title: &str,
  line: &[&str],
  now: Timestamp,
) -> Result<(ResourceId, Version<Tool>), SeedError> {
  let (program, args) = line.split_first().expect("a command line names its program");
  let action = CommandAction::new(
    CommandProgram::new(program).expect("a valid program"),
    args
      .iter()
      .map(|arg| CommandArg::new(arg).expect("a valid argument"))
      .collect(),
  );
  publish(db, title, vec![Tool::command("", action)], now)
}

fn pinned(
  id: ResourceId,
  version: &Version<Pipeline>,
  profiles: &Profiles,
  tools: &BTreeMap<ResourceId, Version<Tool>>,
) -> Result<Published, SeedError> {
  let snapshot = RunSnapshot::build(
    id,
    version,
    PublishedVersions {
      profiles: &profiles.versions,
      tools,
      instructions: &BTreeMap::new(),
      skills: &BTreeMap::new(),
    },
  )?;
  Ok(Published { snapshot })
}

/// An agent connection of `provider` named `title` that is stored but never checked or run: what a
/// scenario's recorded runs name as their CLI.
pub fn recorded_connection(
  db: &mut ProjectDb,
  (title, provider, program): (&str, AgentProvider, &Path),
  now: Timestamp,
) -> Result<ConnectionId, SeedError> {
  let connection = AgentConnection::new(
    ConnectionId::generate(),
    name(title),
    provider,
    program.to_path_buf(),
    now,
  )?;
  let connection = connection.into_connection();
  db.create_connection(&connection)?;
  Ok(connection.id())
}

/// The catalogue of the Usage page's scenario: the design's, its Explorer and Builder on Codex (`gpt-5.5`), its
/// Planner and Reviewer on Claude Code (`claude-sonnet-5-5`), so runs spend on both CLIs.
pub fn usage(
  db: &mut ProjectDb,
  (codex, claude): (ConnectionId, ConnectionId),
  now: Timestamp,
) -> Result<Catalogue, SeedError> {
  let profiles = Profiles::publish_on(
    db,
    |role| match role {
      "Planner" | "Reviewer" => (claude, CLAUDE_MODEL),
      _ => (codex, MODEL),
    },
    now,
  )?;
  starter_catalogue(db, &profiles, now)
}

/// The model of the usage scenario's Claude Code profiles.
const CLAUDE_MODEL: &str = "claude-sonnet-5-5";

/// The catalogue the design shows: Feature delivery v3 (the starter pipeline: Research, Plan with plan
/// approval, Implement, Run checks and Review, Create pull request) and Quick fix v2 (Research, Plan,
/// Implement, Review). Its tools run `cargo test` and `gh pr create`, which the QA repository cannot
/// pass: runs here are recorded, not run.
pub fn kontur(db: &mut ProjectDb, connection: ConnectionId, now: Timestamp) -> Result<Catalogue, SeedError> {
  let profiles = Profiles::publish(db, connection, now)?;
  starter_catalogue(db, &profiles, now)
}

/// Feature delivery v3 and Quick fix v2 on `profiles`, with the checks and pull request tools.
fn starter_catalogue(db: &mut ProjectDb, profiles: &Profiles, now: Timestamp) -> Result<Catalogue, SeedError> {
  let checks = command_tool(db, "Run checks", CHECKS, now)?;
  let pull_request = command_tool(db, "Create pull request", PULL_REQUEST, now)?;
  let assignments = StarterAssignments {
    explorer: Some(profiles.explorer),
    planner: Some(profiles.planner),
    builder: Some(profiles.builder),
    reviewer: Some(profiles.reviewer),
    checks: Some(checks.0),
    pull_request: Some(pull_request.0),
  };
  let versions = [
    "Implement the plan.",
    "Implement the approved plan.",
    "Implement the approved plan in small commits.",
  ]
  .into_iter()
  .map(|implement| feature_delivery(|step| starter_text(step, implement), assignments))
  .collect();
  let feature = publish(db, "Feature delivery", versions, now)?;
  let quick_fix = publish(
    db,
    "Quick fix",
    vec![quick_fix(profiles, 1), quick_fix(profiles, 2)],
    now,
  )?;
  let tools = BTreeMap::from([checks, pull_request]);
  Ok(Catalogue {
    feature_delivery: pinned(feature.0, &feature.1, profiles, &tools)?,
    quick_fix: pinned(quick_fix.0, &quick_fix.1, profiles, &tools)?,
  })
}

/// The catalogue of live runs with `orchester-fake-codex`, agent steps only: Feature delivery (Research,
/// Plan with plan approval, Implement, Review) and Quick fix (Implement). Every step's prompt carries the
/// task's brief, so a brief that makes the fake ask asks once per step: Quick fix asks once.
pub fn live(db: &mut ProjectDb, connection: ConnectionId, now: Timestamp) -> Result<Catalogue, SeedError> {
  let profiles = Profiles::publish(db, connection, now)?;
  let research = agent_step(
    ("research", "Research"),
    profiles.explorer,
    RepositoryAccess::ReadOnly,
    &[],
  );
  let mut plan = agent_step(
    ("plan", "Plan"),
    profiles.planner,
    RepositoryAccess::ReadOnly,
    &["research"],
  );
  plan.approval_gate = true;
  let implement = agent_step(
    ("implement", "Implement"),
    profiles.builder,
    RepositoryAccess::Write,
    &["plan"],
  );
  let review = agent_step(
    ("review", "Review"),
    profiles.reviewer,
    RepositoryAccess::ReadOnly,
    &["implement"],
  );
  let feature = Pipeline::new(vec![research, plan, implement, review]).expect("the live pipeline is valid");
  let quick = Pipeline::new(vec![agent_step(
    ("implement", "Implement"),
    profiles.builder,
    RepositoryAccess::Write,
    &[],
  )])
  .expect("a one-step pipeline is valid");
  let feature = publish(db, "Feature delivery", vec![feature], now)?;
  let quick_fix = publish(db, "Quick fix", vec![quick], now)?;
  let tools = BTreeMap::new();
  Ok(Catalogue {
    feature_delivery: pinned(feature.0, &feature.1, &profiles, &tools)?,
    quick_fix: pinned(quick_fix.0, &quick_fix.1, &profiles, &tools)?,
  })
}

/// The command line the tool of step `step` runs, as its transcript shows it.
pub fn command_line(step: &str) -> String {
  if step == StarterStep::RunChecks.key() {
    CHECKS
  } else {
    PULL_REQUEST
  }
  .join(" ")
}

fn starter_text(step: StarterStep, implement: &str) -> StarterStepText {
  let (title, instructions) = match step {
    StarterStep::Research => ("Research", "Find what the task touches and how it is tested."),
    StarterStep::Plan => ("Plan", "Write the plan for the change, step by step."),
    StarterStep::Implement => ("Implement", implement),
    StarterStep::RunChecks => ("Run checks", ""),
    StarterStep::Review => ("Review", "Review the change against the acceptance criteria."),
    StarterStep::CreatePullRequest => ("Create pull request", ""),
  };
  StarterStepText {
    name: name(title),
    instructions: PromptText::new(instructions).expect("the starter's instructions are valid prompts"),
  }
}

/// Quick fix at `version`: version 2 lets Review send the work back to Implement once.
fn quick_fix(profiles: &Profiles, version: u32) -> Pipeline {
  let mut review = agent_step(
    ("review", "Review"),
    profiles.reviewer,
    RepositoryAccess::ReadOnly,
    &["implement"],
  );
  if version > 1 {
    review.on_failure = Some(FailureRoute {
      return_to: StepKey::new("implement").expect("a valid step key"),
      max_retries: std::num::NonZeroU32::MIN,
    });
  }
  Pipeline::new(vec![
    agent_step(
      ("research", "Research"),
      profiles.explorer,
      RepositoryAccess::ReadOnly,
      &[],
    ),
    agent_step(
      ("plan", "Plan"),
      profiles.planner,
      RepositoryAccess::ReadOnly,
      &["research"],
    ),
    agent_step(
      ("implement", "Implement"),
      profiles.builder,
      RepositoryAccess::Write,
      &["plan"],
    ),
    review,
  ])
  .expect("Quick fix is a valid pipeline")
}

fn agent_step((key, title): (&str, &str), profile: ResourceId, repository: RepositoryAccess, after: &[&str]) -> Step {
  Step {
    key: StepKey::new(key).expect("a valid step key"),
    name: name(title),
    kind: StepKind::Agent {
      profile: Some(profile),
      repository,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("an empty prompt is valid"),
    depends_on: after
      .iter()
      .map(|key| StepKey::new(key).expect("a valid step key"))
      .collect(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }
}
