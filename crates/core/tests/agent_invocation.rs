//! How agent steps start the Codex CLI, which environment it sees, which installed copy is used,
//! and how its token usage is counted and shown.

use std::{
  ffi::OsString,
  path::{Path, PathBuf},
};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  connection::{
    AgentProvider, compare_versions,
    invocation::{CodexInvocation, Platform, agent_environment},
    rank_installed,
  },
  resource::RepositoryAccess,
  run::{TokenUsage, compact_tokens},
};

fn invocation(access: RepositoryAccess, platform: Platform) -> CodexInvocation<'static> {
  CodexInvocation {
    executable: Path::new("/bin/codex"),
    model: "gpt-5.5",
    reasoning_effort: Some("low"),
    access,
    worktree: Path::new("/repo/.orchester/worktrees/run"),
    platform,
  }
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
    .command("Do it".into(), environment)
    .expect("unicode worktree");
  assert_eq!(command.program(), Path::new("/bin/codex"));
  assert_eq!(command.current_dir(), Some(Path::new("/repo/.orchester/worktrees/run")));
  assert_eq!(command.stdin(), Some("Do it"));
  assert!(!command.inherits_env());
  let names: Vec<&str> = command.env().iter().map(|(name, _)| name.as_str()).collect();
  assert_eq!(names, ["PATH", "Path", "HOME", "CODEX_HOME", "SystemRoot"]);
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
      success: true,
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
