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
    invocation::{CODEX_ISOLATION, CodexAgentFiles, CodexInvocation, Platform, agent_environment, is_agent_variable},
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
    repository: Path::new("/repo"),
    access,
    network: NetworkAccess::Off,
    writable_roots: &[],
    worktree: Path::new("/repo/.orchester/worktrees/run"),
    platform,
    mcp: None,
    agent_files: CodexAgentFiles::Loaded,
  }
}

/// The arguments after the isolation overrides and the untrusted projects, which every invocation
/// starts with.
fn args(invocation: &CodexInvocation<'_>) -> Vec<String> {
  let args = invocation.args().expect("unicode paths");
  let isolation: Vec<String> = CODEX_ISOLATION
    .iter()
    .flat_map(|setting| ["-c".to_owned(), (*setting).to_owned()])
    .collect();
  assert_eq!(args[2..2 + isolation.len()], isolation, "{args:?}");
  let rest = &args[2 + isolation.len()..];
  assert!(rest[1].starts_with("projects={"), "{args:?}");
  [&args[..2], &rest[2..]].concat()
}

/// The `-c projects=…` value of an invocation in `worktree` of `repository` on `platform`.
fn untrusted(worktree: &str, repository: &str, platform: Platform) -> String {
  let invocation = CodexInvocation {
    repository: Path::new(repository),
    worktree: Path::new(worktree),
    platform,
    ..invocation(RepositoryAccess::Write, platform)
  };
  let args = invocation.args().expect("unicode paths");
  let at = args
    .iter()
    .position(|arg| arg.starts_with("projects="))
    .expect("the untrusted projects");
  assert_eq!(args[at - 1], "-c");
  args[at].clone()
}

/// The repository's own `.codex` folder never loads: the worktree and its repository are untrusted
/// whatever the user's configuration trusts, written as Codex names them (on Windows: `\`, no
/// `\\?\` prefix, a lower-case drive letter), as an inline table that merges with the user's.
#[test]
fn invocation_marks_the_worktree_and_its_repository_untrusted() {
  assert_eq!(
    untrusted("/repo/.orchester/worktrees/run", "/repo", Platform::MacOs),
    r#"projects={"/repo/.orchester/worktrees/run"={trust_level="untrusted"},"/repo"={trust_level="untrusted"}}"#
  );
  assert_eq!(
    untrusted(
      r"\\?\H:\Projects\Kontur\.orchester\worktrees\run",
      "H:/Projects/Kontur",
      Platform::Windows
    ),
    r#"projects={"h:\\Projects\\Kontur\\.orchester\\worktrees\\run"={trust_level="untrusted"},"h:\\Projects\\Kontur"={trust_level="untrusted"}}"#
  );
  assert_eq!(
    untrusted(
      r"\\?\UNC\server\share\repo\wt",
      r"\\server\share\repo",
      Platform::Windows
    ),
    r#"projects={"\\\\server\\share\\repo\\wt"={trust_level="untrusted"},"\\\\server\\share\\repo"={trust_level="untrusted"}}"#
  );
  // Equal keys appear once: a key twice in an inline table would stop Codex from starting.
  assert_eq!(
    untrusted(r"\\?\H:\Repo", "h:/Repo", Platform::Windows),
    r#"projects={"h:\\Repo"={trust_level="untrusted"}}"#
  );
}

/// What App Server would otherwise take from the user's own Codex setup is turned off on every start:
/// the notify program, plugins and apps, and the permission requests that could stay granted.
#[test]
fn invocation_turns_off_what_the_users_codex_setup_would_add() {
  let args = invocation(RepositoryAccess::Write, Platform::MacOs)
    .args()
    .expect("unicode paths");
  for setting in [
    "notify=[]",
    "features.plugins=false",
    "features.apps=false",
    "features.exec_permission_approvals=false",
    "features.request_permissions_tool=false",
  ] {
    assert!(
      args.windows(2).any(|pair| pair == ["-c", setting]),
      "{setting}: {args:?}"
    );
  }
}

#[test]
fn app_server_keeps_normal_codex_home_and_open_stdin_without_bypass_flags() {
  let command = invocation(RepositoryAccess::Write, Platform::Windows)
    .command(
      [(OsString::from("CODEX_HOME"), OsString::from("/user/.codex"))],
      &CommandEnvironment::default(),
    )
    .expect("unicode paths");
  assert!(command.interactive_stdin());
  assert!(command.stdin().is_none());
  assert_eq!(command.args()[..4], ["app-server", "--stdio", "-c", "notify=[]"]);
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

#[test]
fn invocation_read_only_has_no_writable_root_and_the_windows_sandbox_on_windows_only() {
  for platform in [Platform::Windows, Platform::MacOs] {
    let mut expected = vec!["app-server", "--stdio"];
    if platform == Platform::Windows {
      expected.extend(["-c", "windows.sandbox=elevated"]);
    }
    expected.extend(["-c", "sandbox_workspace_write.writable_roots=[]"]);
    assert_eq!(args(&invocation(RepositoryAccess::ReadOnly, platform)), expected);
  }
}

#[test]
fn invocation_write_names_the_worktree_as_writable_root_and_its_network() {
  let roots = "sandbox_workspace_write.writable_roots=[\"/repo/.orchester/worktrees/run\"]";
  assert_eq!(
    args(&invocation(RepositoryAccess::Write, Platform::Windows)),
    [
      "app-server",
      "--stdio",
      "-c",
      "windows.sandbox=elevated",
      "-c",
      "sandbox_workspace_write.network_access=false",
      "-c",
      roots
    ]
  );
  assert_eq!(
    args(&invocation(RepositoryAccess::Write, Platform::MacOs)),
    [
      "app-server",
      "--stdio",
      "-c",
      "sandbox_workspace_write.network_access=false",
      "-c",
      roots
    ]
  );
}

#[test]
fn invocation_never_uses_bypass_flags_or_the_noninteractive_exec() {
  for access in [RepositoryAccess::ReadOnly, RepositoryAccess::Write] {
    let args = args(&invocation(access, Platform::Windows));
    assert!(
      args
        .iter()
        .all(|arg| !arg.contains("dangerously") && arg != "--full-auto" && arg != "exec" && arg != "never")
    );
  }
}

#[test]
fn invocation_network_on_opens_the_workspace_write_sandbox_network_only() {
  let network = "sandbox_workspace_write.network_access=true";
  for platform in [Platform::Windows, Platform::MacOs] {
    let mut write = invocation(RepositoryAccess::Write, platform);
    assert!(!args(&write).iter().any(|arg| arg == network));
    write.network = NetworkAccess::On;
    let on = args(&write);
    let at = on.iter().position(|arg| arg == network).expect("network flag");
    assert_eq!(on[at - 1], "-c");
    // The read-only sandbox has no network setting.
    let mut read_only = invocation(RepositoryAccess::ReadOnly, platform);
    read_only.network = NetworkAccess::On;
    assert!(!args(&read_only).iter().any(|arg| arg.contains("network_access")));
  }
}

#[test]
fn invocation_command_runs_in_the_worktree_with_a_clean_environment() {
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
    .command(environment, &CommandEnvironment::default())
    .expect("unicode worktree");
  assert_eq!(command.program(), Path::new("/bin/codex"));
  assert_eq!(command.current_dir(), Some(Path::new("/repo/.orchester/worktrees/run")));
  assert!(
    command.interactive_stdin(),
    "the prompt goes in the turn, not on standard input"
  );
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
    .command(environment, &project)
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

/// The token of the app's own MCP server (`docs/design/app-mcp-connection.md`, Security › Agents in runs) and
/// the variable naming a QA copy's discovery folder stay with the person: a Codex agent starts without them,
/// in any spelling Windows would match, on either platform.
#[test]
fn app_mcp_variables_are_not_passed_to_codex() {
  let environment = [
    ("PATH", "/usr/bin"),
    ("ORCHESTER_APP_MCP_TOKEN", "app-token"),
    ("Orchester_App_Mcp_Token", "app-token"),
    ("ORCHESTER_APP_MCP_DISCOVERY", "/repo/.tmp/qa/task/mcp"),
    ("CODEX_HOME", "/home/me/.codex"),
  ];
  for name in [
    "ORCHESTER_APP_MCP_TOKEN",
    "orchester_app_mcp_token",
    "ORCHESTER_APP_MCP_DISCOVERY",
  ] {
    assert!(!is_agent_variable(name), "{name}");
  }
  for platform in [Platform::Windows, Platform::MacOs] {
    let command = invocation(RepositoryAccess::Write, platform)
      .command(
        environment.map(|(name, value)| (OsString::from(name), OsString::from(value))),
        &CommandEnvironment::default(),
      )
      .expect("unicode worktree");
    let names: Vec<&str> = command.env().iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["PATH", "CODEX_HOME"], "{platform:?}");
    assert!(
      !command.env().iter().any(|(_, value)| value == "app-token"),
      "{platform:?}"
    );
  }
}
