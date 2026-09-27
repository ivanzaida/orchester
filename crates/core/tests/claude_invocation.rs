//! How agent steps start the Claude Code CLI: its arguments per effective permissions and session,
//! and the environment it sees. Claude Code enforces the same options on Windows and macOS (its own
//! sandbox, the only platform-specific part, is not used), so every expectation is checked for both.

use std::{collections::BTreeMap, ffi::OsString, path::Path};

use orchester_core::{
  connection::{
    AgentProvider,
    invocation::{ClaudeInvocation, ClaudeSession, Platform},
  },
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
    capability::EnforcedPermissions,
  },
  resource::{CommandEnvironment, VariableName},
};
use uuid::Uuid;

const SESSION: Uuid = Uuid::from_u128(0x0199_a0a0_0000_7000_8000_0000_0000_0001);
const PLATFORMS: [Platform; 2] = [Platform::Windows, Platform::MacOs];

fn allowed(values: &[&str]) -> CommandPermission {
  CommandPermission::Allowed(
    AllowedCommands::new(
      values
        .iter()
        .map(|value| CommandPrefix::new(value).expect("valid prefix"))
        .collect(),
    )
    .expect("valid list"),
  )
}

fn enforced(
  repository: RepositoryAccess,
  commands: CommandPermission,
  network: NetworkAccess,
  platform: Platform,
) -> EnforcedPermissions {
  AgentPermissions::new(repository, commands, network)
    .enforced_on(AgentProvider::ClaudeCodeCli, platform)
    .expect("enforceable on Claude Code")
}

fn invocation(permissions: &EnforcedPermissions) -> ClaudeInvocation<'_> {
  ClaudeInvocation {
    executable: Path::new("/bin/claude"),
    model: "haiku",
    effort: Some("low"),
    permissions,
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

const BASE: [&str; 12] = [
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
];

/// [`BASE`], `permissions`, then the new session.
fn expected(permissions: &[&str]) -> Vec<String> {
  BASE
    .iter()
    .chain(permissions)
    .chain(&["--session-id", "0199a0a0-0000-7000-8000-000000000001"])
    .map(|arg| (*arg).to_owned())
    .collect()
}

#[test]
fn claude_invocation_read_only_without_commands_offers_no_tool_that_can_change_files() {
  for platform in PLATFORMS {
    let permissions = enforced(RepositoryAccess::ReadOnly, allowed(&[]), NetworkAccess::Off, platform);
    assert_eq!(
      args(&invocation(&permissions)),
      expected(&[
        "--tools",
        "Read,Glob,Grep,TodoWrite",
        "--permission-mode",
        "dontAsk",
        "--disallowedTools",
        "Edit",
        "Write",
        "NotebookEdit",
        "Bash",
        "WebFetch",
        "WebSearch",
      ])
    );
  }
}

#[test]
fn claude_invocation_read_only_runs_only_the_listed_commands() {
  for platform in PLATFORMS {
    let permissions = enforced(
      RepositoryAccess::ReadOnly,
      allowed(&["git log", "rg"]),
      NetworkAccess::On,
      platform,
    );
    assert_eq!(
      args(&invocation(&permissions)),
      expected(&[
        "--tools",
        "Read,Glob,Grep,TodoWrite,Bash,WebFetch,WebSearch",
        "--permission-mode",
        "dontAsk",
        "--disallowedTools",
        "Edit",
        "Write",
        "NotebookEdit",
        "--allowedTools",
        "Bash(git log:*)",
        "Bash(rg:*)",
      ])
    );
  }
}

#[test]
fn claude_invocation_write_accepts_edits_and_allows_the_listed_commands() {
  for platform in PLATFORMS {
    let permissions = enforced(
      RepositoryAccess::Write,
      allowed(&["git", "pnpm test"]),
      NetworkAccess::Off,
      platform,
    );
    // Network off cannot be enforced for commands, so it stays on; the web tools still go.
    assert!(permissions.network_stays_on().is_some());
    assert_eq!(
      args(&invocation(&permissions)),
      expected(&[
        "--tools",
        "Read,Glob,Grep,TodoWrite,Edit,Write,NotebookEdit,Bash",
        "--permission-mode",
        "acceptEdits",
        "--disallowedTools",
        "WebFetch",
        "WebSearch",
        "--allowedTools",
        "Bash(git:*)",
        "Bash(pnpm test:*)",
      ])
    );
  }
}

#[test]
fn claude_invocation_refuses_what_claude_code_cannot_enforce() {
  for platform in PLATFORMS {
    let any = AgentPermissions::new(
      RepositoryAccess::Write,
      CommandPermission::AnyInWorktree,
      NetworkAccess::On,
    );
    assert!(any.enforced_on(AgentProvider::ClaudeCodeCli, platform).is_err());
  }
}

#[test]
fn claude_invocation_never_bypasses_permissions_or_loads_the_users_setup() {
  for repository in [RepositoryAccess::ReadOnly, RepositoryAccess::Write] {
    let permissions = enforced(repository, allowed(&["git"]), NetworkAccess::On, Platform::current());
    let args = args(&invocation(&permissions));
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
  let permissions = enforced(
    RepositoryAccess::Write,
    allowed(&["git"]),
    NetworkAccess::On,
    Platform::current(),
  );
  let mut resume = invocation(&permissions);
  resume.session = ClaudeSession::Resume("5f0c2a1e-7b3d-4c8e-9a61-2d4b8e0f1a37");
  let args = args(&resume);
  assert_eq!(
    &args[args.len() - 2..],
    ["--resume", "5f0c2a1e-7b3d-4c8e-9a61-2d4b8e0f1a37"]
  );
  assert!(!args.contains(&"--session-id".to_owned()));
  assert!(args.contains(&"acceptEdits".to_owned()), "permissions are given again");
  assert!(matches!(ClaudeSession::continuing(None), ClaudeSession::New(_)));
  assert_eq!(ClaudeSession::continuing(Some("s")), ClaudeSession::Resume("s"));
}

#[test]
fn claude_invocation_without_an_effort_leaves_the_default() {
  let permissions = enforced(
    RepositoryAccess::ReadOnly,
    allowed(&[]),
    NetworkAccess::On,
    Platform::current(),
  );
  let mut plain = invocation(&permissions);
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
  let permissions = enforced(
    RepositoryAccess::Write,
    allowed(&["git"]),
    NetworkAccess::On,
    Platform::current(),
  );
  let command = invocation(&permissions)
    .command("Do it".into(), environment, &CommandEnvironment::default())
    .expect("unicode worktree");
  assert_eq!(command.program(), Path::new("/bin/claude"));
  assert_eq!(command.current_dir(), Some(Path::new("/repo/.orchester/worktrees/run")));
  assert_eq!(command.stdin(), Some("Do it"));
  assert!(!command.inherits_env());
  let names: Vec<&str> = command.env().iter().map(|(name, _)| name.as_str()).collect();
  assert_eq!(names, ["PATH", "USERPROFILE", "CLAUDE_CONFIG_DIR"]);
}

#[test]
fn claude_invocation_command_adds_the_project_variables() {
  let permissions = enforced(
    RepositoryAccess::Write,
    allowed(&["git"]),
    NetworkAccess::On,
    Platform::current(),
  );
  let name = VariableName::new("CARGO_TARGET_DIR").expect("a valid variable name");
  let project =
    CommandEnvironment::new(BTreeMap::from([(name, "/repo/.orchester/target".to_owned())])).expect("valid variables");
  let environment = [(OsString::from("PATH"), OsString::from("/usr/bin"))];
  let command = invocation(&permissions)
    .command("Do it".into(), environment, &project)
    .expect("unicode worktree");
  assert_eq!(
    command.env(),
    [
      ("PATH".to_owned(), "/usr/bin".to_owned()),
      ("CARGO_TARGET_DIR".to_owned(), "/repo/.orchester/target".to_owned()),
    ]
  );
}
