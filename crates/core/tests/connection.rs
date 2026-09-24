//! Connection checks against output captured from the real CLIs (codex-cli 0.139.0, Claude Code
//! 2.1.89), replayed through a fake command runner so the CLIs are not needed.

use std::{
  collections::HashMap,
  ffi::OsString,
  path::{Path, PathBuf},
};

use orchester_core::{
  connection::{
    AgentProvider, CatalogSource, CheckResult, CommandError, CommandOutput, CommandRunner, Connection, ConnectionError,
    ConnectionStatus, ModelSupport, SignIn, check, executable_candidates, parse_claude_auth_status,
    parse_codex_catalog, parse_codex_login_status, parse_version,
  },
  id::ConnectionId,
  name::Name,
  resource::ModelId,
  time::Timestamp,
};

/// Shape of `codex debug models --bundled`, shortened to the fields Orchester reads plus a few
/// it ignores.
const CODEX_CATALOG: &str = r#"{
  "models": [
    {"slug": "gpt-5.5", "display_name": "GPT-5.5", "visibility": "list", "priority": 0,
     "default_reasoning_level": "medium",
     "supported_reasoning_levels": [
       {"effort": "low", "description": "Fast responses"},
       {"effort": "medium", "description": "Balanced"},
       {"effort": "high", "description": "Deeper reasoning"},
       {"effort": "xhigh", "description": "Maximum reasoning"}]},
    {"slug": "gpt-5.4-mini", "display_name": "GPT-5.4-Mini", "visibility": "list", "priority": 4,
     "supported_reasoning_levels": [{"effort": "low"}, {"effort": "medium"}]},
    {"slug": "codex-auto-review", "display_name": "Codex Auto Review", "visibility": "hide", "priority": 29,
     "supported_reasoning_levels": [{"effort": "low"}]}
  ]
}"#;

fn ok(stdout: &str) -> CommandOutput {
  CommandOutput {
    success: true,
    stdout: stdout.to_owned(),
    stderr: String::new(),
  }
}

fn failed(stdout: &str, stderr: &str) -> CommandOutput {
  CommandOutput {
    success: false,
    stdout: stdout.to_owned(),
    stderr: stderr.to_owned(),
  }
}

/// Answers each argument list with a captured output; anything else was never allowed to run.
struct Replay(HashMap<String, Result<CommandOutput, CommandError>>);

impl Replay {
  fn new(answers: impl IntoIterator<Item = (&'static str, Result<CommandOutput, CommandError>)>) -> Self {
    Self(
      answers
        .into_iter()
        .map(|(args, answer)| (args.to_owned(), answer))
        .collect(),
    )
  }
}

impl CommandRunner for Replay {
  fn run(&self, _program: &Path, args: &[&str]) -> Result<CommandOutput, CommandError> {
    let key = args.join(" ");
    self
      .0
      .get(&key)
      .cloned()
      .unwrap_or_else(|| panic!("`{key}` is not one of the allowed read-only commands"))
  }
}

fn executable() -> PathBuf {
  std::env::current_dir().expect("current directory").join("codex.exe")
}

fn now() -> Timestamp {
  Timestamp::from_unix_millis(1_700_000_000_000)
}

#[test]
fn version_parsing_recognizes_each_cli() {
  assert_eq!(
    parse_version(AgentProvider::CodexCli, &ok("codex-cli 0.139.0\n")).as_deref(),
    Some("0.139.0")
  );
  assert_eq!(
    parse_version(AgentProvider::ClaudeCodeCli, &ok("2.1.89 (Claude Code)\n")).as_deref(),
    Some("2.1.89")
  );
  assert_eq!(
    parse_version(AgentProvider::ClaudeCodeCli, &ok("codex-cli 0.139.0")),
    None
  );
  assert_eq!(
    parse_version(AgentProvider::CodexCli, &failed("codex-cli 0.139.0", "")),
    None
  );
}

#[test]
fn codex_login_status_parsing() {
  assert_eq!(
    parse_codex_login_status(&ok("Logged in using ChatGPT\n")),
    SignIn::SignedIn
  );
  let on_stderr = CommandOutput {
    success: true,
    stdout: String::new(),
    stderr: "Logged in using an API key - sk-***\n".into(),
  };
  assert_eq!(parse_codex_login_status(&on_stderr), SignIn::SignedIn);
  assert_eq!(
    parse_codex_login_status(&failed("", "Not logged in\n")),
    SignIn::SignedOut
  );
  assert_eq!(
    parse_codex_login_status(&failed("", "error: unexpected\n")),
    SignIn::Unknown
  );
}

#[test]
fn claude_auth_status_parsing() {
  assert_eq!(
    parse_claude_auth_status(&ok(r#"{"loggedIn": true, "authMethod": "claude.ai"}"#)),
    SignIn::SignedIn
  );
  assert_eq!(
    parse_claude_auth_status(&failed(
      r#"{"loggedIn": false, "authMethod": "none", "apiProvider": "firstParty"}"#,
      ""
    )),
    SignIn::SignedOut
  );
  assert_eq!(parse_claude_auth_status(&ok("Logged in")), SignIn::Unknown);
}

#[test]
fn codex_catalog_lists_visible_models_with_their_efforts() {
  let models = parse_codex_catalog(CODEX_CATALOG).expect("valid catalog");
  let ids: Vec<&str> = models.iter().map(|model| model.id().as_str()).collect();
  assert_eq!(ids, ["gpt-5.5", "gpt-5.4-mini"]);
  assert_eq!(models[0].efforts(), ["low", "medium", "high", "xhigh"]);
  assert_eq!(parse_codex_catalog("not json"), None);
}

#[test]
fn check_codex_reads_version_sign_in_and_catalog() {
  let runner = Replay::new([
    ("--version", Ok(ok("codex-cli 0.139.0\n"))),
    ("login status", Ok(ok("Logged in using ChatGPT\n"))),
    ("debug models --bundled", Ok(ok(CODEX_CATALOG))),
  ]);
  let check = check(AgentProvider::CodexCli, &executable(), &runner, now());
  let CheckResult::Available {
    version,
    sign_in,
    catalog,
  } = check.result
  else {
    panic!("expected an available CLI, got {:?}", check.result);
  };
  assert_eq!(version, "0.139.0");
  assert_eq!(sign_in, SignIn::SignedIn);
  assert_eq!(catalog.source(), CatalogSource::Provider);
  assert_eq!(catalog.models().len(), 2);
}

#[test]
fn check_claude_uses_documented_aliases() {
  let runner = Replay::new([
    ("--version", Ok(ok("2.1.89 (Claude Code)\n"))),
    ("auth status --json", Ok(failed(r#"{"loggedIn": false}"#, ""))),
  ]);
  let mut connection = connection(AgentProvider::ClaudeCodeCli);
  connection.record_check(check(AgentProvider::ClaudeCodeCli, &executable(), &runner, now()));
  assert_eq!(connection.status(), ConnectionStatus::SignInRequired);
  let catalog = connection.catalog().expect("a successful check has a catalog");
  assert_eq!(catalog.source(), CatalogSource::DocumentedAliases);
  let ids: Vec<&str> = connection.models().iter().map(|model| model.id().as_str()).collect();
  assert_eq!(ids, ["opus", "sonnet"]);
  let custom = ModelId::new("claude-sonnet-4-6").expect("valid model id");
  assert_eq!(catalog.support(&custom), ModelSupport::NotVerified);
  assert_eq!(
    catalog.support(&ModelId::new("opus").expect("valid model id")),
    ModelSupport::Listed
  );
}

#[test]
fn check_failures_become_statuses() {
  let cases = [
    (Err(CommandError::NotFound), ConnectionStatus::ExecutableMissing),
    (Err(CommandError::TimedOut), ConnectionStatus::CouldNotRun),
    (Ok(ok("git version 2.45.0")), ConnectionStatus::NotRecognized),
  ];
  for (answer, expected) in cases {
    let runner = Replay::new([("--version", answer)]);
    let mut connection = connection(AgentProvider::CodexCli);
    connection.record_check(check(AgentProvider::CodexCli, &executable(), &runner, now()));
    assert_eq!(connection.status(), expected);
    assert!(connection.models().is_empty());
  }
}

#[test]
fn sign_in_status_that_cannot_run_is_unknown() {
  let runner = Replay::new([
    ("--version", Ok(ok("codex-cli 0.139.0"))),
    ("login status", Err(CommandError::TimedOut)),
    ("debug models --bundled", Ok(failed("", "boom"))),
  ]);
  let mut connection = connection(AgentProvider::CodexCli);
  connection.record_check(check(AgentProvider::CodexCli, &executable(), &runner, now()));
  assert_eq!(connection.status(), ConnectionStatus::SignInUnknown);
  assert!(connection.status().is_usable());
  assert!(connection.models().is_empty());
}

fn connection(provider: AgentProvider) -> Connection {
  Connection::new(
    ConnectionId::generate(),
    Name::new("Codex").expect("valid name"),
    provider,
    executable(),
    now(),
  )
  .expect("absolute executable")
}

#[test]
fn changing_the_executable_drops_the_last_check() {
  let runner = Replay::new([("--version", Err(CommandError::NotFound))]);
  let mut connection = connection(AgentProvider::CodexCli);
  connection.record_check(check(AgentProvider::CodexCli, &executable(), &runner, now()));
  assert_eq!(connection.status(), ConnectionStatus::ExecutableMissing);
  connection
    .set_executable(executable().with_file_name("other.exe"))
    .expect("absolute path");
  assert_eq!(connection.status(), ConnectionStatus::NotChecked);
  assert_eq!(
    connection.set_executable(PathBuf::from("codex")),
    Err(ConnectionError::RelativeExecutable(PathBuf::from("codex")))
  );
}

#[test]
fn candidates_follow_the_path_order() {
  let first = std::env::current_dir().expect("current directory").join("a");
  let second = std::env::current_dir().expect("current directory").join("b");
  let path: OsString = std::env::join_paths([&first, &second]).expect("joinable paths");
  let candidates = executable_candidates(AgentProvider::ClaudeCodeCli, Some(&path));
  assert!(candidates[0].starts_with(&first));
  assert!(candidates.last().expect("candidates").starts_with(&second));
  assert!(
    candidates
      .iter()
      .all(|candidate| candidate.file_stem().is_some_and(|stem| stem == "claude"))
  );
  assert!(executable_candidates(AgentProvider::CodexCli, None).is_empty());
}
