//! What running a task needs besides the state machine: the prompt, the branch name, the
//! preflight and the task phase.

use std::{collections::BTreeMap, num::NonZeroU32};

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogSource, CheckResult, Connection, ConnectionCheck, ConnectionStatus,
    ModelCatalog, SignIn,
  },
  id::{ConnectionId, ProjectId, ResourceId, TaskId},
  name::Name,
  project::{Project, ProjectKey, TaskKey},
  resource::{AgentProfile, Pipeline, PromptText, ProviderSettings, Step, StepKind, Variable, VersionNumber},
  run::{
    BranchName, PipelineInfo, PreflightInput, PreflightProblem, PromptAssemblyError, PromptInput, PromptSource,
    TaskPhase, preflight,
  },
  task::{Task, TaskDetails, TaskStatus},
};

use crate::fixture::{Config, Logged, at, key, model, version};

fn task_key(number: u32) -> TaskKey {
  TaskKey::new(
    ProjectKey::new("ORC").expect("valid key"),
    NonZeroU32::new(number).expect("positive"),
  )
}

fn task(brief: &str, criteria: &[&str]) -> Task {
  let details = TaskDetails {
    title: Name::new("Add GitHub sign-in").expect("valid title"),
    brief: brief.into(),
    acceptance_criteria: criteria.iter().map(|criterion| (*criterion).to_owned()).collect(),
    pipeline: None,
    limits: None,
  };
  Task::draft(TaskId::generate(), task_key(12), details, at(1))
}

fn project() -> Project {
  Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("valid key"),
    Name::new("Kontur").expect("valid name"),
  )
}

fn builder(instructions: &str) -> AgentProfile {
  AgentProfile::new(
    Name::new("Builder").expect("valid role"),
    "Implements the approved plan.".into(),
    PromptText::new(instructions).expect("valid prompt"),
    ConnectionId::generate(),
    model("gpt-5.5"),
    ProviderSettings::default(),
  )
}

fn step_with(instructions: &str) -> Step {
  let config = Config::feature_delivery();
  let mut step = config
    .pipeline_version
    .body()
    .step(&key("implement"))
    .expect("step")
    .clone();
  step.instructions = PromptText::new(instructions).expect("valid prompt");
  step
}

fn assemble(profile: &AgentProfile, step: &Step, task: &Task) -> Result<String, PromptAssemblyError> {
  let logged = Logged::started();
  let project = project();
  PromptInput {
    project: &project,
    task,
    run: &logged.run,
    execution: logged.exec("implement"),
    pipeline: PipelineInfo {
      id: ResourceId::generate(),
      name: "Feature delivery",
      version: VersionNumber::new(NonZeroU32::new(3).expect("positive")),
    },
    step,
    profile_id: ResourceId::generate(),
    profile,
  }
  .assemble()
}

#[test]
fn prompt_assembles_role_step_and_task_in_order() {
  let prompt = assemble(
    &builder("Keep commits small for {{task.id}}."),
    &step_with("Work on {{run.branch}}, attempt {{ run.attempt }}. Literal \\{{task.title}}."),
    &task(
      "Users sign in with GitHub.",
      &["Sign-in works", "Cancel shows an error"],
    ),
  )
  .expect("every variable has a value");
  let expected = "# Your role: Builder\n\nImplements the approved plan.\n\nKeep commits small for ORC-12.\n\n# Current \
                  step: implement\n\nWork on orchester/orc-129, attempt 1. Literal {{task.title}}.\n\n# Task ORC-12: \
                  Add GitHub sign-in\n\nUsers sign in with GitHub.\n\n## Acceptance criteria\n- Sign-in works\n- \
                  Cancel shows an error\n";
  assert_eq!(prompt, expected);
}

#[test]
fn prompt_names_a_variable_without_a_value_instead_of_leaving_it_empty() {
  let error = assemble(
    &builder("Fine."),
    &step_with("See {{task.attachments}}."),
    &task("Brief.", &[]),
  );
  assert_eq!(
    error,
    Err(PromptAssemblyError::UnavailableVariable {
      variable: Variable::TaskAttachments,
      part: PromptSource::StepInstructions,
    })
  );
  // The budget in force, as a whole number of tokens.
  let prompt = assemble(&builder("Budget {{run.budget}}"), &step_with(""), &task("Brief.", &[]));
  assert!(prompt.expect("run.budget is known").contains("Budget 1400000"));
}

#[test]
fn branch_for_run_uses_the_lowercase_task_key_and_ordinal() {
  let branch = BranchName::for_run(&task_key(128), NonZeroU32::new(2).expect("positive"));
  assert_eq!(branch.as_str(), "orchester/orc-128-run-2");
}

fn connection(provider: AgentProvider, sign_in: SignIn) -> Connection {
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Codex").expect("valid name"),
    provider,
    std::env::temp_dir().join("codex"),
    at(1),
  )
  .expect("absolute path");
  connection.record_check(ConnectionCheck {
    at: at(2),
    result: CheckResult::Available {
      version: "0.139.0".into(),
      sign_in,
      catalog: ModelCatalog::new(CatalogSource::Provider, Vec::new()),
    },
  });
  connection.into_connection()
}

/// Preflight of the sample pipeline with every profile on `connection`.
fn problems_with(connection: &Connection) -> Vec<PreflightProblem> {
  let mut config = Config::feature_delivery();
  for published in config.profiles.values_mut() {
    let body = published.body();
    let moved = AgentProfile::new(
      body.role().clone(),
      String::new(),
      body.instructions().clone(),
      connection.id(),
      body.model().clone(),
      ProviderSettings::default(),
    );
    *published = version(published.number().get(), moved);
  }
  preflight(PreflightInput {
    pipeline: Some(config.pipeline_version.body()),
    profiles: &config.profiles,
    connections: std::slice::from_ref(connection),
  })
}

#[test]
fn preflight_reports_approval_gates_and_unusable_connections() {
  let signed_in = connection(AgentProvider::CodexCli, SignIn::SignedIn);
  assert_eq!(
    problems_with(&signed_in),
    [PreflightProblem::ApprovalGate { step: key("plan") }]
  );
  let signed_out = connection(AgentProvider::CodexCli, SignIn::SignedOut);
  assert!(
    problems_with(&signed_out).contains(&PreflightProblem::ConnectionNotUsable {
      step: key("research"),
      status: ConnectionStatus::SignInRequired,
    })
  );
  let claude = connection(AgentProvider::ClaudeCodeCli, SignIn::SignedIn);
  assert!(problems_with(&claude).contains(&PreflightProblem::UnsupportedProvider { step: key("review") }));
}

#[test]
fn preflight_reports_tool_steps_unpublished_profiles_and_missing_pipelines() {
  let config = Config::feature_delivery();
  let mut steps = config.pipeline_version.body().steps().to_vec();
  steps[0].kind = StepKind::Tool { tool: None };
  let pipeline = Pipeline::new(steps).expect("valid pipeline");
  let problems = preflight(PreflightInput {
    pipeline: Some(&pipeline),
    profiles: &BTreeMap::new(),
    connections: &[],
  });
  assert_eq!(problems[0], PreflightProblem::ToolStep { step: key("research") });
  assert!(problems.contains(&PreflightProblem::ProfileNotPublished { step: key("implement") }));
  let none = preflight(PreflightInput {
    pipeline: None,
    profiles: &config.profiles,
    connections: &[],
  });
  assert_eq!(none, [PreflightProblem::PipelineNotPublished]);
}

#[test]
fn task_phase_follows_the_task_and_its_latest_run() {
  let draft = task("Brief.", &["Works"]);
  assert_eq!(TaskPhase::of(&draft, None), TaskPhase::Draft);
  let queued = Task::from_stored(
    draft.id(),
    draft.key().clone(),
    draft.details().clone(),
    TaskStatus::Queued,
    draft.created_at(),
  );
  assert_eq!(TaskPhase::of(&queued, None), TaskPhase::Queued);
  let mut logged = Logged::pending();
  assert_eq!(TaskPhase::of(&queued, Some(&logged.run)), TaskPhase::Queued);
  logged.apply(|run| run.start(at(11)));
  assert_eq!(TaskPhase::of(&queued, Some(&logged.run)), TaskPhase::Running);
  assert!(TaskPhase::of(&queued, Some(&logged.run)).is_active());
  let research = logged.exec("research");
  logged.apply(|run| run.start_step(research, at(12)));
  logged.apply(|run| run.fail_step(research, at(13)));
  assert_eq!(TaskPhase::of(&queued, Some(&logged.run)), TaskPhase::Failed);
}
