//! How agent steps start the Codex CLI, which environment it sees, which installed copy is used,
//! and how its token usage is counted and shown.

use std::{
  collections::BTreeMap,
  ffi::OsString,
  path::{Path, PathBuf},
};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  connection::{
    AgentProvider, compare_versions,
    invocation::{CodexAgentFiles, CodexInvocation, Platform, agent_environment},
    rank_installed,
  },
  permission::NetworkAccess,
  resource::{CommandEnvironment, RepositoryAccess, VariableName},
  run::TokenUsage,
  tokens::compact_tokens,
};

fn invocation(access: RepositoryAccess, platform: Platform) -> CodexInvocation<'static> {
  CodexInvocation {
    executable: Path::new("/bin/codex"),
    model: "gpt-5.5",
    reasoning_effort: Some("low"),
    access,
    network: NetworkAccess::Off,
    writable_roots: &[],
    worktree: Path::new("/repo/.orchester/worktrees/run"),
    platform,
    resume: None,
    output_schema: None,
    mcp: None,
    agent_files: CodexAgentFiles::Loaded,
  }
}

#[test]
fn app_server_keeps_normal_codex_home_and_open_stdin_without_bypass_flags() {
  let command = invocation(RepositoryAccess::Write, Platform::Windows)
    .app_server_command(
      [(OsString::from("CODEX_HOME"), OsString::from("/user/.codex"))],
      &CommandEnvironment::default(),
    )
    .expect("unicode paths");
  assert!(command.interactive_stdin());
  assert!(command.stdin().is_none());
  assert_eq!(command.args()[..2], ["app-server", "--stdio"]);
  assert!(command.args().contains(&"windows.sandbox=elevated".to_owned()));
  assert!(!command.args().iter().any(|arg| arg.contains("dangerously")));
  assert_eq!(
    command
      .env()
      .iter()
      .find(|(name, _)| name == "CODEX_HOME")
      .map(|(_, value)| value.as_str()),
    Some("/user/.codex")
  );
  assert_eq!(command.env().iter().filter(|(name, _)| name == "CODEX_HOME").count(), 1);
}

fn args(invocation: &CodexInvocation<'_>) -> Vec<String> {
  invocation
    .args()
    .into_iter()
    .map(|arg| arg.into_string().expect("unicode"))
    .collect()
}

const BASE: [&str; 11] = [
  "-a",
  "never",
  "exec",
  "--json",
  "--ignore-user-config",
  "--ignore-rules",
  "-m",
  "gpt-5.5",
  "-c",
  "model_reasoning_effort=low",
  "--sandbox",
];

#[test]
fn invocation_read_only_uses_the_read_only_sandbox_on_every_platform() {
  for platform in [Platform::Windows, Platform::MacOs] {
    let mut expected: Vec<&str> = BASE.to_vec();
    expected.extend(["read-only", "-C", "/repo/.orchester/worktrees/run", "-"]);
    assert_eq!(args(&invocation(RepositoryAccess::ReadOnly, platform)), expected);
  }
}

#[test]
fn invocation_write_confines_writes_with_the_windows_sandbox_on_windows_only() {
  let mut windows: Vec<&str> = BASE.to_vec();
  windows.extend([
    "workspace-write",
    "-c",
    "windows.sandbox=elevated",
    "-C",
    "/repo/.orchester/worktrees/run",
    "-",
  ]);
  assert_eq!(args(&invocation(RepositoryAccess::Write, Platform::Windows)), windows);
  let mut mac: Vec<&str> = BASE.to_vec();
  mac.extend(["workspace-write", "-C", "/repo/.orchester/worktrees/run", "-"]);
  assert_eq!(args(&invocation(RepositoryAccess::Write, Platform::MacOs)), mac);
}

#[test]
fn invocation_never_uses_bypass_flags_and_puts_the_approval_flag_before_exec() {
  for access in [RepositoryAccess::ReadOnly, RepositoryAccess::Write] {
    let args = args(&invocation(access, Platform::Windows));
    assert!(
      args
        .iter()
        .all(|arg| !arg.contains("dangerously") && arg != "--full-auto")
    );
    assert_eq!(&args[..3], ["-a", "never", "exec"]);
  }
}

#[test]
fn invocation_resume_continues_the_session_with_the_sandbox_as_configuration() {
  let session = "01a0d831-b556-7730-be9d-2491a9359250";
  let mut resume = invocation(RepositoryAccess::Write, Platform::Windows);
  resume.resume = Some(session);
  assert_eq!(
    args(&resume),
    [
      "-a",
      "never",
      "exec",
      "resume",
      "--json",
      "--ignore-user-config",
      "--ignore-rules",
      "-m",
      "gpt-5.5",
      "-c",
      "model_reasoning_effort=low",
      "-c",
      "sandbox_mode=workspace-write",
      "-c",
      "windows.sandbox=elevated",
      session,
      "-",
    ]
  );
  let mut read_only = invocation(RepositoryAccess::ReadOnly, Platform::MacOs);
  read_only.resume = Some(session);
  let read_only = args(&read_only);
  assert!(read_only.contains(&"sandbox_mode=read-only".to_owned()));
  assert!(
    !read_only
      .iter()
      .any(|arg| arg == "-C" || arg == "--sandbox" || arg.contains("dangerously"))
  );
  let command = resume
    .command("Continue.".to_owned(), Vec::new(), &CommandEnvironment::default())
    .expect("unicode");
  assert_eq!(command.current_dir(), Some(Path::new("/repo/.orchester/worktrees/run")));
}

#[test]
fn invocation_output_schema_is_given_to_new_sessions_only() {
  let schema = Path::new("/repo/.orchester/step-result.schema.json");
  let mut new = invocation(RepositoryAccess::ReadOnly, Platform::MacOs);
  new.output_schema = Some(schema);
  let new_args = args(&new);
  let at = new_args
    .iter()
    .position(|arg| arg == "--output-schema")
    .expect("a new session reports its result in the schema");
  assert_eq!(new_args[at + 1], "/repo/.orchester/step-result.schema.json");
  assert!(at < new_args.iter().position(|arg| arg == "-C").expect("worktree"));
  assert_eq!(new_args.last().map(String::as_str), Some("-"));
  new.resume = Some("session");
  assert!(!args(&new).contains(&"--output-schema".to_owned()));
}

#[test]
fn invocation_network_on_opens_the_workspace_write_sandbox_network_only() {
  let network = "sandbox_workspace_write.network_access=true";
  for platform in [Platform::Windows, Platform::MacOs] {
    let mut write = invocation(RepositoryAccess::Write, platform);
    assert!(!args(&write).iter().any(|arg| arg == network));
    write.network = NetworkAccess::On;
    let new_session = args(&write);
    let at = new_session.iter().position(|arg| arg == network).expect("network flag");
    assert_eq!(new_session[at - 1], "-c");
    write.resume = Some("01a0d831-b556-7730-be9d-2491a9359250");
    assert!(args(&write).iter().any(|arg| arg == network));
    // The read-only sandbox has no network setting.
    let mut read_only = invocation(RepositoryAccess::ReadOnly, platform);
    read_only.network = NetworkAccess::On;
    assert!(!args(&read_only).iter().any(|arg| arg.contains("network_access")));
  }
}

#[test]
fn invocation_without_an_effort_leaves_the_default() {
  let mut plain = invocation(RepositoryAccess::ReadOnly, Platform::MacOs);
  plain.reasoning_effort = None;
  assert!(!args(&plain).iter().any(|arg| arg.starts_with("model_reasoning_effort")));
}

#[test]
fn invocation_command_runs_in_the_worktree_with_the_prompt_on_stdin_and_a_clean_environment() {
  let environment = [
    ("PATH", "/usr/bin"),
    ("Path", "C:\\Windows"),
    ("HOME", "/home/me"),
    ("CLAUDECODE", "1"),
    ("ANTHROPIC_BASE_URL", "http://proxy"),
    ("GH_TOKEN", "secret"),
    ("GITHUB_TOKEN", "secret"),
    ("OPENAI_API_KEY", "secret"),
    ("CODEX_HOME", "/home/me/.codex"),
    ("SystemRoot", "C:\\Windows"),
  ]
  .map(|(name, value)| (OsString::from(name), OsString::from(value)));
  let command = invocation(RepositoryAccess::Write, Platform::Windows)
    .command("Do it".into(), environment, &CommandEnvironment::default())
    .expect("unicode worktree");
  assert_eq!(command.program(), Path::new("/bin/codex"));
  assert_eq!(command.current_dir(), Some(Path::new("/repo/.orchester/worktrees/run")));
  assert_eq!(command.stdin(), Some("Do it"));
  assert!(!command.inherits_env());
  let names: Vec<&str> = command.env().iter().map(|(name, _)| name.as_str()).collect();
  assert_eq!(names, ["PATH", "Path", "HOME", "CODEX_HOME", "SystemRoot"]);
}

#[test]
fn invocation_command_adds_the_project_variables_over_the_allowed_ones() {
  let environment = [("Path", "C:\\Windows"), ("GH_TOKEN", "secret"), ("HOME", "/home/me")]
    .map(|(name, value)| (OsString::from(name), OsString::from(value)));
  let project = CommandEnvironment::new(BTreeMap::from([
    (variable("CARGO_TARGET_DIR"), "/repo/.orchester/target".to_owned()),
    (variable("PATH"), "/opt/tools".to_owned()),
  ]))
  .expect("valid variables");
  let command = invocation(RepositoryAccess::Write, Platform::Windows)
    .command("Do it".into(), environment, &project)
    .expect("unicode worktree");
  assert_eq!(
    command.env(),
    [
      ("HOME".to_owned(), "/home/me".to_owned()),
      ("CARGO_TARGET_DIR".to_owned(), "/repo/.orchester/target".to_owned()),
      ("PATH".to_owned(), "/opt/tools".to_owned()),
    ]
  );
}

fn variable(name: &str) -> VariableName {
  VariableName::new(name).expect("a valid variable name")
}

#[test]
fn invocation_environment_drops_what_is_not_allowed() {
  let kept = agent_environment([
    (OsString::from("CLAUDE_CODE_SESSION_ID"), OsString::from("x")),
    (OsString::from("tmp"), OsString::from("C:\\Temp")),
  ]);
  assert_eq!(kept, [("tmp".to_owned(), "C:\\Temp".to_owned())]);
}

/// Answers `--version` per program path.
struct Versions(Vec<(&'static str, Result<&'static str, CommandError>)>);

impl CommandRunner for Versions {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    let (_, answer) = self
      .0
      .iter()
      .find(|(path, _)| Path::new(path) == command.program())
      .expect("a known program");
    answer.clone().map(|stdout| CommandOutput {
      code: Some(0),
      stdout: stdout.into(),
      stderr: String::new(),
    })
  }
}

#[test]
fn discovery_ranks_installed_copies_newest_first() {
  let runner = Versions(vec![
    ("/a/codex.exe", Ok("codex-cli 0.137.0")),
    ("/b/codex.cmd", Ok("codex-cli 0.139.0")),
    ("/c/codex", Err(CommandError::NotFound)),
    ("/d/codex", Ok("codex-cli 0.99.2")),
  ]);
  let candidates = ["/a/codex.exe", "/b/codex.cmd", "/c/codex", "/d/codex"].map(PathBuf::from);
  let ranked = rank_installed(AgentProvider::CodexCli, candidates.to_vec(), &runner);
  let order: Vec<_> = ranked
    .iter()
    .map(|installed| {
      (
        installed.executable.to_str().expect("utf-8"),
        installed.version.as_deref(),
      )
    })
    .collect();
  assert_eq!(
    order,
    [
      ("/b/codex.cmd", Some("0.139.0")),
      ("/a/codex.exe", Some("0.137.0")),
      ("/d/codex", Some("0.99.2")),
      ("/c/codex", None),
    ]
  );
  assert!(compare_versions(Some("0.139.0"), Some("0.137.10")).is_gt());
}

#[test]
fn tokens_count_input_without_cache_plus_output() {
  let usage = TokenUsage {
    input: 281_000,
    cached_input: 200_000,
    output: 34_000,
  };
  assert_eq!(usage.counted_input(), 81_000);
  assert_eq!(usage.counted(), 115_000);
  let total: TokenUsage = [usage, usage].into_iter().sum();
  assert_eq!(total.counted(), 230_000);
}

#[test]
fn tokens_compact_follows_the_display_rule() {
  let cases = [
    (0, "0"),
    (980, "980"),
    (999, "999"),
    (1_000, "1k"),
    (12_500, "12.5k"),
    (18_000, "18k"),
    (18_240, "18.2k"),
    (99_949, "99.9k"),
    (99_950, "100k"),
    (250_000, "250k"),
    (412_345, "412k"),
    (999_499, "999k"),
    (999_960, "1M"),
    (1_400_000, "1.4M"),
    (20_000_000, "20M"),
    (3_849_999, "3.8M"),
    (1_250_000_000, "1.3B"),
  ];
  for (count, text) in cases {
    assert_eq!(compact_tokens(count), text, "{count}");
  }
}
