//! What running a task needs besides the state machine and the prompt: the branch name, the
//! preflight and the task phase.

use std::{collections::BTreeMap, num::NonZeroU32};

use orchester_core::{
  connection::{
    AgentConnection, AgentEndpoint, AgentProvider, ApiEndpoint, BaseUrl, CatalogSource, CheckResult, Connection,
    ConnectionCheck, ConnectionKind, ConnectionStatus, EndpointPreset, GitHubHost, ModelCatalog, SignIn,
  },
  id::{ConnectionId, RunId, SecretId},
  integration::{MergeKind, RunMerge},
  name::Name,
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess, StepCommands,
    capability::{PermissionOption, PermissionProblem, Unavailable},
  },
  resource::{AgentProfile, Pipeline, ProviderSettings, StepKind},
  run::{BranchName, PreflightInput, PreflightProblem, StepFailure, TaskPhase, preflight},
  task::{Task, TaskStatus},
};

use crate::fixture::{Config, Logged, at, key, revision, task, task_key, version};

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
  connection
    .record_check(ConnectionCheck {
      at: at(2),
      result: CheckResult::Available {
        version: "0.139.0".into(),
        sign_in,
        catalog: ModelCatalog::new(CatalogSource::Provider, Vec::new()),
      },
    })
    .expect("an agent check fits a CLI connection");
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
    AgentEndpoint::Program(std::env::temp_dir().join("gh")),
    at(1),
  )
  .expect("absolute path");
  assert!(problems_with(&github, &before).contains(&PreflightProblem::UnsupportedProvider { step: key("review") }));
}

#[test]
fn preflight_runs_anthropic_native_steps_once_checked() {
  let native = |provider, url: &str| {
    let endpoint = ApiEndpoint::new(
      EndpointPreset::Official,
      BaseUrl::new(url).expect("valid base URL"),
      SecretId::generate(),
    );
    AgentConnection::native(
      ConnectionId::generate(),
      Name::new("Native API").expect("valid name"),
      provider,
      endpoint,
      at(1),
    )
    .expect("native connection")
  };
  let permissions = AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::Allowed(AllowedCommands::default()),
    NetworkAccess::On,
  );
  let openai = native(AgentProvider::OpenAiApi, "https://api.openai.com/v1").into_connection();
  let problems = problems_with(&openai, &permissions);
  assert!(problems.contains(&PreflightProblem::NativeNotAvailable { step: key("review") }));
  assert!(
    problems
      .iter()
      .all(|problem| matches!(problem, PreflightProblem::NativeNotAvailable { .. }))
  );
  let mut anthropic = native(AgentProvider::AnthropicApi, "https://api.anthropic.com");
  assert!(
    problems_with(&anthropic.clone().into_connection(), &permissions).contains(
      &PreflightProblem::ConnectionNotUsable {
        step: key("review"),
        status: ConnectionStatus::NotChecked,
      }
    )
  );
  anthropic
    .record_check(ConnectionCheck {
      at: at(2),
      result: CheckResult::Api { balance: None },
    })
    .expect("an API check fits a native connection");
  assert_eq!(problems_with(&anthropic.into_connection(), &permissions), []);
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
fn preflight_refuses_a_command_list_on_a_profile_that_asks() {
  let config = Config::feature_delivery();
  let mut steps = config.pipeline_version.body().steps().to_vec();
  let StepKind::Agent { permissions, .. } = &mut steps[0].kind else {
    panic!("research is an agent step");
  };
  permissions.commands = StepCommands::Only(
    AllowedCommands::new(vec![CommandPrefix::new("git log").expect("valid command")]).expect("valid list"),
  );
  let pipeline = Pipeline::new(steps).expect("valid pipeline");
  let claude = connection(AgentProvider::ClaudeCodeCli, SignIn::SignedIn);
  let with_commands = |commands: CommandPermission| {
    let mut profiles = config.profiles.clone();
    for published in profiles.values_mut() {
      let body = published.body();
      let moved = AgentProfile::new(
        body.role().clone(),
        String::new(),
        body.instructions().clone(),
        claude.id(),
        body.model().clone(),
        ProviderSettings::default(),
      )
      .with_permissions(AgentPermissions::new(
        RepositoryAccess::ReadOnly,
        commands.clone(),
        NetworkAccess::On,
      ));
      *published = version(published.number().get(), moved);
    }
    preflight(PreflightInput {
      pipeline: Some(&pipeline),
      profiles: &profiles,
      tools: &BTreeMap::new(),
      connections: std::slice::from_ref(&claude),
    })
  };
  let asking = with_commands(CommandPermission::AskBefore);
  assert_eq!(
    asking,
    [PreflightProblem::CommandListNotOffered { step: key("research") }],
    "a list on a profile that asks would let asking offer what the list leaves out"
  );
  let listed = with_commands(CommandPermission::Allowed(
    AllowedCommands::new(vec![CommandPrefix::new("git log").expect("valid command")]).expect("valid list"),
  ));
  assert!(listed.is_empty(), "{listed:?}");
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
  assert_eq!(TaskPhase::of(&draft, None, None), TaskPhase::Draft);
  let queued = Task::from_stored(
    draft.id(),
    draft.key().clone(),
    draft.details().clone(),
    TaskStatus::Queued,
    None,
    draft.created_at(),
  );
  assert_eq!(TaskPhase::of(&queued, None, None), TaskPhase::Queued);
  let mut logged = Logged::pending();
  assert_eq!(TaskPhase::of(&queued, Some(&logged.run), None), TaskPhase::Queued);
  logged.apply(|run| run.start(at(11)));
  assert_eq!(TaskPhase::of(&queued, Some(&logged.run), None), TaskPhase::Running);
  assert!(TaskPhase::of(&queued, Some(&logged.run), None).is_active());
  assert_eq!(
    TaskPhase::of(&queued, Some(&logged.run), None).without_runner(),
    TaskPhase::Interrupted
  );
  let research = logged.exec("research");
  logged.apply(|run| run.start_step(research, at(12)));
  logged.apply(|run| run.fail_step(research, StepFailure::ReportedFailed, at(13)));
  assert_eq!(TaskPhase::of(&queued, Some(&logged.run), None), TaskPhase::Failed);
  assert_eq!(TaskPhase::Failed.without_runner(), TaskPhase::Failed);
  assert_eq!(TaskPhase::Waiting.without_runner(), TaskPhase::Waiting);
  assert_eq!(TaskPhase::Queued.without_runner(), TaskPhase::Queued);
}

#[test]
fn task_phase_is_merged_once_the_run_that_succeeded_is_merged() {
  let task = task("Brief.", &["Works"]);
  let mut logged = Logged::started();
  logged.through_plan();
  for step in ["implement", "checks", "review", "pr"] {
    logged.pass(step);
  }
  assert_eq!(TaskPhase::of(&task, Some(&logged.run), None), TaskPhase::Succeeded);
  let merge = RunMerge {
    run: logged.run.id(),
    into: BranchName::new("main").expect("valid branch"),
    revision: revision('b'),
    kind: MergeKind::FastForward,
    merged_at: at(30),
  };
  assert_eq!(TaskPhase::of(&task, Some(&logged.run), Some(&merge)), TaskPhase::Merged);
  let earlier = RunMerge {
    run: RunId::generate(),
    ..merge
  };
  assert_eq!(
    TaskPhase::of(&task, Some(&logged.run), Some(&earlier)),
    TaskPhase::Succeeded,
    "the merge of another run does not count"
  );
}
