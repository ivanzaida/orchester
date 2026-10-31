//! Every Claude Code process Orchester starts runs without the person's auto memory, which Claude Code keeps per git
//! repository and so shares with a run's worktree; neither the environment nor the project can turn it back on.
//! Codex is started as before. `cargo test -p orchester-core --test auto_memory auto_memory`.

use std::{collections::BTreeMap, ffi::OsString, path::Path};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  connection::{
    AgentProvider,
    invocation::{
      CLAUDE_NO_AUTO_MEMORY, ClaudeChat, ClaudeInvocation, ClaudeSession, CodexAgentFiles, CodexInvocation, Platform,
    },
    plan_limits::read::claude_check_command,
    reviewer::{ClaudeReviewer, CliEstimator, CodexReviewer, EstimatorCli, ReviewFolder},
  },
  name::Name,
  permission::{AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess},
  resource::{CommandEnvironment, ModelId, VariableName},
  run::review::{ReviewInput, ReviewedRequest, ReviewedStep, ReviewedTask},
  task::{TaskDetails, TaskLimits, estimate::EstimateInput},
};
use uuid::Uuid;

const OFF: &str = "0";

/// The environment Orchester runs in, with auto memory turned on as a person's shell might.
fn environment() -> Vec<(OsString, OsString)> {
  vec![
    ("PATH".into(), "/bin".into()),
    (CLAUDE_NO_AUTO_MEMORY.0.into(), OFF.into()),
  ]
}

/// The project's variables, turning auto memory on in another spelling.
fn project() -> CommandEnvironment {
  let name = VariableName::new(&CLAUDE_NO_AUTO_MEMORY.0.to_ascii_lowercase()).expect("a valid name");
  CommandEnvironment::new(BTreeMap::from([(name, OFF.to_owned())])).expect("valid variables")
}

/// An agent environment the project's variables were added to, as the runner gives the one-request CLIs.
fn variables() -> Vec<(String, String)> {
  vec![
    ("PATH".to_owned(), "/bin".to_owned()),
    (CLAUDE_NO_AUTO_MEMORY.0.to_owned(), OFF.to_owned()),
  ]
}

/// The values `command` gives the auto memory switch, in any spelling.
fn switch(command: &Command) -> Vec<&str> {
  command
    .env()
    .iter()
    .filter(|(name, _)| name.eq_ignore_ascii_case(CLAUDE_NO_AUTO_MEMORY.0))
    .map(|(_, value)| value.as_str())
    .collect()
}

fn assert_off(command: &Command, what: &str) {
  assert!(!command.inherits_env(), "{what}: the environment is spelled out");
  assert_eq!(switch(command), ["1"], "{what}: {command:?}");
  assert!(
    command
      .secret_env()
      .iter()
      .all(|secret| !secret.name().eq_ignore_ascii_case(CLAUDE_NO_AUTO_MEMORY.0)),
    "{what}"
  );
}

struct Silent;

impl CommandRunner for Silent {
  fn run(&self, _: &Command) -> Result<CommandOutput, CommandError> {
    Err(CommandError::NotFound)
  }
}

fn model() -> ModelId {
  ModelId::new("claude-haiku-4-5-20251001").expect("model")
}

fn folder() -> ReviewFolder {
  ReviewFolder {
    work: "/tmp/review/work".into(),
    instructions: "/tmp/review/instructions.md".into(),
    schema: "/tmp/review/schema.json".into(),
  }
}

fn details() -> TaskDetails {
  TaskDetails {
    title: Name::new("Refresh the fixtures").expect("title"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::NONE,
  }
}

fn review_input() -> ReviewInput {
  ReviewInput::new(
    ReviewedRequest {
      command: true,
      server: "command".to_owned(),
      action: "cargo build".to_owned(),
      arguments: None,
      note: None,
      working_folder: None,
    },
    ReviewedStep {
      name: "Implement".to_owned(),
      repository: RepositoryAccess::Write,
      network: NetworkAccess::Off,
      writable_folders: Vec::new(),
    },
    ReviewedTask {
      title: "Refresh the fixtures".to_owned(),
      brief: String::new(),
      criteria: Vec::new(),
    },
    &[],
  )
  .expect("within the size of a review")
}

#[test]
fn auto_memory_is_off_for_every_claude_code_step() {
  for platform in [Platform::Windows, Platform::MacOs] {
    let permissions = AgentPermissions::new(
      RepositoryAccess::Write,
      CommandPermission::Allowed(AllowedCommands::default()),
      NetworkAccess::Off,
    )
    .enforced_on(AgentProvider::ClaudeCodeCli, platform)
    .expect("enforceable on Claude Code");
    let command = ClaudeInvocation {
      executable: Path::new("/bin/claude"),
      model: "haiku",
      effort: None,
      permissions: &permissions,
      writable_roots: &[],
      worktree: Path::new("/repo/.orchester/worktrees/run"),
      session: ClaudeSession::New(Uuid::nil()),
      mcp: None,
      skills: None,
      code_index: None,
      approvals: Some(Path::new("/tmp/approvals.json")),
      checks: &[],
    }
    .command(("Go.", &[]), environment(), &project())
    .expect("a command");
    assert_off(&command, "step");
  }
}

#[test]
fn auto_memory_is_off_for_the_orchestrator_chat() {
  let command = ClaudeChat {
    executable: Path::new("/bin/claude"),
    model: "haiku",
    effort: None,
    folder: Path::new("/work/kontur"),
    session: ClaudeSession::New(Uuid::nil()),
    system_prompt: Path::new("/tmp/prompt.md"),
    mcp_config: Path::new("/tmp/mcp.json"),
    denied: &[],
    denied_files: &[],
  }
  .command(("Hello.", &[]), environment(), &project())
  .expect("a command");
  assert_off(&command, "chat");
}

#[test]
fn auto_memory_is_off_for_the_reviewer_the_estimator_and_the_limits_check() {
  let reviewer = ClaudeReviewer::new(
    Path::new("/bin/claude"),
    (model(), None),
    (folder(), variables()),
    Silent,
  );
  assert_off(&reviewer.command(&review_input()), "reviewer");
  let estimator = CliEstimator {
    cli: EstimatorCli::ClaudeCode,
    executable: Path::new("/bin/claude").to_path_buf(),
    model: (model(), None),
    folder: folder(),
    environment: variables(),
  };
  assert_off(&estimator.command(&EstimateInput::new(&details(), &[])), "estimator");
  let check = claude_check_command(Path::new("/bin/claude"), variables(), Path::new("/tmp/check"));
  assert_off(&check, "plan limits check");
}

#[test]
fn auto_memory_switch_is_not_given_to_codex() {
  let step = CodexInvocation {
    executable: Path::new("/bin/codex"),
    repository: Path::new("/repo"),
    access: RepositoryAccess::Write,
    network: NetworkAccess::Off,
    writable_roots: &[],
    worktree: Path::new("/repo/.orchester/worktrees/run"),
    platform: Platform::current(),
    mcp: None,
    agent_files: CodexAgentFiles::Loaded,
  }
  .command(Vec::<(OsString, OsString)>::new(), &CommandEnvironment::default())
  .expect("a command");
  assert!(switch(&step).is_empty(), "{step:?}");
  let reviewer = CodexReviewer::new(Path::new("/bin/codex"), (model(), None), (folder(), Vec::new()), Silent);
  assert!(switch(&reviewer.command(&review_input())).is_empty());
  let estimator = CliEstimator {
    cli: EstimatorCli::Codex,
    executable: Path::new("/bin/codex").to_path_buf(),
    model: (model(), None),
    folder: folder(),
    environment: Vec::new(),
  };
  assert!(switch(&estimator.command(&EstimateInput::new(&details(), &[]))).is_empty());
}
