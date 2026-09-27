//! What running a task needs besides the state machine and the prompt: the branch name, the
//! preflight and the task phase.

use std::{collections::BTreeMap, num::NonZeroU32};

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogSource, CheckResult, Connection, ConnectionCheck, ConnectionKind,
    ConnectionStatus, GitHubHost, ModelCatalog, SignIn,
  },
  id::ConnectionId,
  name::Name,
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess, StepCommands,
    capability::{PermissionOption, PermissionProblem, Unavailable},
  },
  resource::{AgentProfile, Pipeline, ProviderSettings, StepKind},
  run::{BranchName, PreflightInput, PreflightProblem, StepFailure, TaskPhase, preflight},
  task::{Task, TaskStatus},
};

use crate::fixture::{Config, Logged, at, key, task, task_key, version};

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
    tools: &BTreeMap::new(),
    connections: std::slice::from_ref(connection),
  })
}

#[test]
fn preflight_allows_approval_gates_and_reports_unusable_connections() {
  let before = AgentPermissions::before_permissions();
  let signed_in = connection(AgentProvider::CodexCli, SignIn::SignedIn);
  // The gated Plan step runs: its result waits for a plan decision.
  assert_eq!(problems_with(&signed_in, &before), []);
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
  assert_eq!(problems_with(&claude, &git), []);
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
    tools: &BTreeMap::new(),
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
    tools: &BTreeMap::new(),
    connections: &[],
  });
  assert_eq!(
    problems[0],
    PreflightProblem::ToolNotPublished { step: key("research") }
  );
  assert!(problems.contains(&PreflightProblem::ProfileNotPublished { step: key("implement") }));
  let none = preflight(PreflightInput {
    pipeline: None,
    profiles: &config.profiles,
    tools: &BTreeMap::new(),
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
