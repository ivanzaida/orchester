//! How agent steps start the Claude Code CLI: its arguments per repository access and session, and
//! the environment it sees. The arguments do not depend on the platform (Claude Code's own sandbox,
//! the only platform-specific part, is not used), so the same expectations hold on Windows and
//! macOS.

use std::{ffi::OsString, path::Path};

use orchester_core::{
  connection::invocation::{ClaudeInvocation, ClaudeSession},
  resource::RepositoryAccess,
};
use uuid::Uuid;

const SESSION: Uuid = Uuid::from_u128(0x0199_a0a0_0000_7000_8000_0000_0000_0001);

fn invocation(access: RepositoryAccess) -> ClaudeInvocation<'static> {
  ClaudeInvocation {
    executable: Path::new("/bin/claude"),
    model: "haiku",
    effort: Some("low"),
    access,
    worktree: Path::new("/repo/.orchester/worktrees/run"),
    session: ClaudeSession::New(SESSION),
  }
}

fn args(invocation: &ClaudeInvocation<'_>) -> Vec<String> {
  invocation
    .args()
    .into_iter()
    .map(|arg| arg.into_string().expect("unicode"))
    .collect()
}

const BASE: [&str; 13] = [
  "-p",
  "--output-format",
  "stream-json",
  "--verbose",
  "--model",
  "haiku",
  "--effort",
  "low",
  "--strict-mcp-config",
  "--setting-sources",
  "",
  "--disable-slash-commands",
  "--tools",
];

/// [`BASE`], then `rest`.
fn expected(rest: &[&str]) -> Vec<String> {
  BASE.iter().chain(rest).map(|arg| (*arg).to_owned()).collect()
}

#[test]
fn claude_invocation_read_only_denies_every_tool_that_can_change_files() {
  assert_eq!(
    args(&invocation(RepositoryAccess::ReadOnly)),
    expected(&[
      "Read,Glob,Grep,TodoWrite",
      "--permission-mode",
      "dontAsk",
      "--disallowedTools",
      "Edit",
      "Write",
      "NotebookEdit",
      "WebFetch",
      "WebSearch",
      "Bash",
      "--session-id",
      "0199a0a0-0000-7000-8000-000000000001",
    ])
  );
}

#[test]
fn claude_invocation_write_accepts_edits_and_allows_git_commands_only() {
  assert_eq!(
    args(&invocation(RepositoryAccess::Write)),
    expected(&[
      "Read,Glob,Grep,TodoWrite,Edit,Write,NotebookEdit,Bash",
      "--permission-mode",
      "acceptEdits",
      "--allowedTools",
      "Bash(git:*)",
      "--session-id",
      "0199a0a0-0000-7000-8000-000000000001",
    ])
  );
}

#[test]
fn claude_invocation_never_bypasses_permissions_or_loads_the_users_setup() {
  for access in [RepositoryAccess::ReadOnly, RepositoryAccess::Write] {
    let args = args(&invocation(access));
    assert!(
      args
        .iter()
        .all(|arg| !arg.contains("dangerously") && arg != "bypassPermissions" && arg != "auto")
    );
    let sources = args
      .iter()
      .position(|arg| arg == "--setting-sources")
      .expect("setting sources");
    assert_eq!(args[sources + 1], "", "no settings file is loaded");
    assert!(args.contains(&"--strict-mcp-config".to_owned()));
    assert!(
      !args.contains(&"--no-session-persistence".to_owned()),
      "sessions stay resumable"
    );
  }
}

#[test]
fn claude_invocation_resume_continues_the_reported_session() {
  let mut resume = invocation(RepositoryAccess::Write);
  resume.session = ClaudeSession::Resume("5f0c2a1e-7b3d-4c8e-9a61-2d4b8e0f1a37");
  let args = args(&resume);
  assert_eq!(
    &args[args.len() - 2..],
    ["--resume", "5f0c2a1e-7b3d-4c8e-9a61-2d4b8e0f1a37"]
  );
  assert!(!args.contains(&"--session-id".to_owned()));
  assert!(args.contains(&"acceptEdits".to_owned()), "permissions are given again");
}

#[test]
fn claude_invocation_without_an_effort_leaves_the_default() {
  let mut plain = invocation(RepositoryAccess::ReadOnly);
  plain.effort = None;
  assert!(!args(&plain).contains(&"--effort".to_owned()));
}

#[test]
fn claude_invocation_command_runs_in_the_worktree_without_the_agent_hosts_variables() {
  let environment = [
    ("PATH", "/usr/bin"),
    ("USERPROFILE", "C:\\Users\\me"),
    ("CLAUDE_CONFIG_DIR", "/home/me/.claude"),
    ("CLAUDECODE", "1"),
    ("CLAUDE_CODE_SESSION_ID", "host-session"),
    ("CLAUDE_CODE_ENTRYPOINT", "sdk-ts"),
    ("CLAUDE_CODE_OAUTH_SCOPES", "user:inference"),
    ("ANTHROPIC_BASE_URL", "http://host-proxy"),
    ("ANTHROPIC_API_KEY", "secret"),
  ]
  .map(|(name, value)| (OsString::from(name), OsString::from(value)));
  let command = invocation(RepositoryAccess::Write)
    .command("Do it".into(), environment)
    .expect("unicode worktree");
  assert_eq!(command.program(), Path::new("/bin/claude"));
  assert_eq!(command.current_dir(), Some(Path::new("/repo/.orchester/worktrees/run")));
  assert_eq!(command.stdin(), Some("Do it"));
  assert!(!command.inherits_env());
  let names: Vec<&str> = command.env().iter().map(|(name, _)| name.as_str()).collect();
  assert_eq!(names, ["PATH", "USERPROFILE", "CLAUDE_CONFIG_DIR"]);
}
