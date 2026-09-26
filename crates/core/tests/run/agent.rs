//! What running a task needs besides the state machine: the prompt, the branch name, the
//! preflight and the task phase.

use std::{collections::BTreeMap, num::NonZeroU32};

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogSource, CheckResult, Connection, ConnectionCheck, ConnectionKind,
    ConnectionStatus, GitHubHost, ModelCatalog, SignIn,
  },
  id::{ConnectionId, ProjectId, ResourceId, TaskId},
  name::Name,
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess, StepCommands,
    capability::{PermissionOption, PermissionProblem, Unavailable},
  },
  project::{Project, ProjectKey, TaskKey},
  resource::{AgentProfile, Pipeline, PromptText, ProviderSettings, Step, StepKind, Variable, VersionNumber},
  run::{
    BranchName, DependencyOutput, PipelineInfo, PreflightInput, PreflightProblem, PromptAssemblyError, PromptInput,
    PromptSource, StepFailure, TaskPhase, preflight,
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
    dependencies: &[],
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
                  Cancel shows an error\n\n# When you finish\n\nEnd with your result: the outcome `done` when the \
                  step is complete, or `failed` when you could not complete it, and a short summary of what you did \
                  for the steps that run after this one.\n";
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
fn prompt_lists_the_outputs_of_the_steps_it_runs_after_with_their_sources() {
  let config = Config::feature_delivery();
  let body = config.pipeline_version.body();
  let (checks, review) = (
    body.step(&key("checks")).expect("step"),
    body.step(&key("review")).expect("step"),
  );
  let pr = body.step(&key("pr")).expect("step");
  let logged = Logged::started();
  let (project, task, profile) = (project(), task("Brief.", &[]), builder("Fine."));
  let attempt = NonZeroU32::new(2).expect("positive");
  let dependencies = [
    DependencyOutput {
      step: checks,
      attempt,
      text: "All 38 tests pass.",
    },
    DependencyOutput {
      step: review,
      attempt,
      text: " ",
    },
  ];
  let prompt = PromptInput {
    project: &project,
    task: &task,
    run: &logged.run,
    execution: logged.exec("pr"),
    pipeline: PipelineInfo {
      id: ResourceId::generate(),
      name: "Feature delivery",
      version: VersionNumber::new(NonZeroU32::new(3).expect("positive")),
    },
    step: pr,
    profile_id: ResourceId::generate(),
    profile: &profile,
    dependencies: &dependencies,
  }
  .assemble()
  .expect("every variable has a value");
  let results = prompt.find("# Results of earlier steps").expect("results section");
  assert!(results > prompt.find("# Task ORC-12").expect("task section"));
  assert!(prompt.contains(
    "
Sources: checks (`checks`, attempt 2), review (`review`, attempt 2)
"
  ));
  assert!(prompt.contains(
    "
## checks (`checks`, attempt 2)

All 38 tests pass.
"
  ));
  assert!(prompt.contains(
    "
## review (`review`, attempt 2)

(no output)
"
  ));
  assert!(prompt.ends_with(
    "for the steps that run after this one.
"
  ));
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

/// Preflight of the sample pipeline with every profile on `connection`, with `permissions`.
fn problems_with(connection: &Connection, permissions: &AgentPermissions) -> Vec<PreflightProblem> {
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
    )
    .with_permissions(permissions.clone());
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
  let before = AgentPermissions::before_permissions();
  let signed_in = connection(AgentProvider::CodexCli, SignIn::SignedIn);
  assert_eq!(
    problems_with(&signed_in, &before),
    [PreflightProblem::ApprovalGate { step: key("plan") }]
  );
  let signed_out = connection(AgentProvider::CodexCli, SignIn::SignedOut);
  assert!(
    problems_with(&signed_out, &before).contains(&PreflightProblem::ConnectionNotUsable {
      step: key("research"),
      status: ConnectionStatus::SignInRequired,
    })
  );
  // Claude Code steps run like Codex ones, with permissions Claude Code can enforce.
  let git = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::Allowed(
      AllowedCommands::new(vec![CommandPrefix::new("git").expect("valid command")]).expect("valid list"),
    ),
    NetworkAccess::On,
  );
  let claude = connection(AgentProvider::ClaudeCodeCli, SignIn::SignedIn);
  assert_eq!(
    problems_with(&claude, &git),
    [PreflightProblem::ApprovalGate { step: key("plan") }]
  );
  assert!(
    problems_with(&claude, &before).contains(&PreflightProblem::PermissionsNotEnforceable {
      step: key("review"),
      reason: PermissionProblem::NotEnforceable {
        option: PermissionOption::AnyCommand,
        reason: Unavailable::CannotConfineCommands,
      },
    })
  );
  let claude_signed_out = connection(AgentProvider::ClaudeCodeCli, SignIn::SignedOut);
  assert!(
    problems_with(&claude_signed_out, &git).contains(&PreflightProblem::ConnectionNotUsable {
      step: key("review"),
      status: ConnectionStatus::SignInRequired,
    })
  );
  // A connection that is not an agent CLI cannot run a step.
  let github = Connection::new(
    ConnectionId::generate(),
    Name::new("GitHub").expect("valid name"),
    ConnectionKind::GitHub {
      host: GitHubHost::github_com(),
    },
    std::env::temp_dir().join("gh"),
    at(1),
  )
  .expect("absolute path");
  assert!(problems_with(&github, &before).contains(&PreflightProblem::UnsupportedProvider { step: key("review") }));
}

#[test]
fn preflight_reports_steps_whose_permissions_codex_cannot_enforce() {
  let config = Config::feature_delivery();
  let mut steps = config.pipeline_version.body().steps().to_vec();
  let StepKind::Agent { permissions, .. } = &mut steps[0].kind else {
    panic!("research is an agent step");
  };
  permissions.commands = StepCommands::Only(
    AllowedCommands::new(vec![CommandPrefix::new("git log").expect("valid command")]).expect("valid list"),
  );
  let pipeline = Pipeline::new(steps).expect("valid pipeline");
  let codex = connection(AgentProvider::CodexCli, SignIn::SignedIn);
  let mut profiles = config.profiles;
  for published in profiles.values_mut() {
    let body = published.body();
    let moved = AgentProfile::new(
      body.role().clone(),
      String::new(),
      body.instructions().clone(),
      codex.id(),
      body.model().clone(),
      ProviderSettings::default(),
    );
    *published = version(published.number().get(), moved);
  }
  let problems = preflight(PreflightInput {
    pipeline: Some(&pipeline),
    profiles: &profiles,
    connections: std::slice::from_ref(&codex),
  });
  assert!(problems.contains(&PreflightProblem::PermissionsNotEnforceable {
    step: key("research"),
    reason: PermissionProblem::NotEnforceable {
      option: PermissionOption::AllowedCommands,
      reason: Unavailable::CannotLimitCommands,
    },
  }));
  assert!(!problems.iter().any(
    |problem| matches!(problem, PreflightProblem::PermissionsNotEnforceable { step, .. } if step != &key("research"))
  ));
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
  assert_eq!(
    TaskPhase::of(&queued, Some(&logged.run)).without_runner(),
    TaskPhase::Interrupted
  );
  let research = logged.exec("research");
  logged.apply(|run| run.start_step(research, at(12)));
  logged.apply(|run| run.fail_step(research, StepFailure::AgentFailed, at(13)));
  assert_eq!(TaskPhase::of(&queued, Some(&logged.run)), TaskPhase::Failed);
  assert_eq!(TaskPhase::Failed.without_runner(), TaskPhase::Failed);
  assert_eq!(TaskPhase::Waiting.without_runner(), TaskPhase::Waiting);
  assert_eq!(TaskPhase::Queued.without_runner(), TaskPhase::Queued);
}
