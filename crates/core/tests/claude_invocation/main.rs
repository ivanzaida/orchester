//! How agent steps start the Claude Code CLI: its arguments per effective permissions and session,
//! and the environment it sees. Claude Code enforces the same options on Windows and macOS (its own
//! sandbox, the only platform-specific part, is not used), so every expectation is checked for both.

use std::{collections::BTreeMap, ffi::OsString, path::Path};

use orchester_core::{
  connection::{
    AgentProvider,
    claude_approval::{ClaudeApprovalScope, PROMPT_TOOL},
    invocation::{ClaudeInvocation, ClaudeSession, InvocationError, Platform, input_line, result_schema},
  },
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
    capability::{EnforcedPermissions, Mechanism, PermissionSetting},
  },
  resource::{CommandEnvironment, VariableName},
  run::STEP_RESULT_SCHEMA,
};
use uuid::Uuid;

mod code_index;

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

/// The Orchester tools that run without asking in a step that only reads, and `ask_question` (it waits for an
/// answer, not an approval), each with its allow rule after the step's other allowed tools.
const READ_RULES: [&str; 8] = [
  "mcp__Orchester__read_file",
  "mcp__Orchester__list_files",
  "mcp__Orchester__search_file",
  "mcp__Orchester__find_references",
  "mcp__Orchester__get_task",
  "mcp__Orchester__save_memory",
  "mcp__Orchester__report_progress",
  "mcp__Orchester__ask_question",
];
/// And in a step that may write.
const WRITE_RULES: [&str; 12] = [
  "mcp__Orchester__read_file",
  "mcp__Orchester__list_files",
  "mcp__Orchester__search_file",
  "mcp__Orchester__edit_file",
  "mcp__Orchester__create_file",
  "mcp__Orchester__delete_file",
  "mcp__Orchester__move_file",
  "mcp__Orchester__find_references",
  "mcp__Orchester__get_task",
  "mcp__Orchester__save_memory",
  "mcp__Orchester__report_progress",
  "mcp__Orchester__ask_question",
];
/// Claude Code's own file tools, which an invocation served Orchester's tools has on its disallowed list, after the
/// ones its permissions deny.
const OWN_FILE_TOOLS: [&str; 9] = [
  "Read",
  "Edit",
  "MultiEdit",
  "Write",
  "NotebookEdit",
  "NotebookRead",
  "Grep",
  "Glob",
  "LS",
];
/// The permission prompt's configuration file, for the invocations that ask a person.
const APPROVALS: &str = "/tmp/approvals.json";

/// An invocation with `permissions`, given the permission prompt when they ask a person.
fn invocation(permissions: &EnforcedPermissions) -> ClaudeInvocation<'_> {
  let asks = ClaudeApprovalScope::of(permissions.permissions(), &[]).is_some();
  ClaudeInvocation {
    executable: Path::new("/bin/claude"),
    model: "haiku",
    effort: Some("low"),
    permissions,
    writable_roots: &[],
    worktree: Path::new("/repo/.orchester/worktrees/run"),
    session: ClaudeSession::New(SESSION),
    mcp: None,
    skills: None,
    code_index: None,
    approvals: asks.then_some(Path::new(APPROVALS)),
    checks: &[],
  }
}

fn args(invocation: &ClaudeInvocation<'_>) -> Vec<String> {
  invocation
    .args()
    .into_iter()
    .map(|arg| arg.into_string().expect("unicode"))
    .collect()
}

const BASE: [&str; 15] = [
  "-p",
  "--input-format",
  "stream-json",
  "--output-format",
  "stream-json",
  "--verbose",
  "--replay-user-messages",
  "--model",
  "haiku",
  "--effort",
  "low",
  "--strict-mcp-config",
  "--setting-sources",
  "",
  "--disable-slash-commands",
];

/// [`BASE`], the result schema, the permission prompt when the step `asks` a person, `permissions`,
/// then the new session.
fn expected(asks: bool, permissions: &[&str]) -> Vec<String> {
  let schema = result_schema();
  let prompt: &[&str] = if asks {
    &["--mcp-config", APPROVALS, "--permission-prompt-tool", PROMPT_TOOL]
  } else {
    &[]
  };
  BASE
    .iter()
    .chain(&["--json-schema", schema.as_str()])
    .chain(prompt)
    .chain(permissions)
    .chain(&["--session-id", "0199a0a0-0000-7000-8000-000000000001"])
    .map(|arg| (*arg).to_owned())
    .collect()
}

#[test]
fn claude_invocation_passes_the_step_result_schema_as_one_json_argument() {
  let permissions = enforced(
    RepositoryAccess::Write,
    allowed(&["git"]),
    NetworkAccess::On,
    Platform::current(),
  );
  for session in [ClaudeSession::New(SESSION), ClaudeSession::Resume("5f0c2a1e")] {
    let mut current = invocation(&permissions);
    current.session = session;
    let args = args(&current);
    let at = args
      .iter()
      .position(|arg| arg == "--json-schema")
      .expect("the schema flag");
    let schema: serde_json::Value = serde_json::from_str(&args[at + 1]).expect("one JSON argument");
    let original: serde_json::Value = serde_json::from_str(STEP_RESULT_SCHEMA).expect("JSON");
    assert_eq!(schema, original);
    assert!(!args[at + 1].contains('\n'));
  }
}

/// Agents ask: without a listed command every command reaches a person; the edit tools stay removed.
#[test]
fn claude_invocation_read_only_without_commands_asks_about_every_command() {
  for platform in PLATFORMS {
    let permissions = enforced(RepositoryAccess::ReadOnly, allowed(&[]), NetworkAccess::Off, platform);
    assert_eq!(
      args(&invocation(&permissions)),
      expected(
        true,
        &[
          "--tools",
          "Read,Glob,Grep,TodoWrite,Bash",
          "--permission-mode",
          "default",
          "--settings",
          r#"{"permissions":{"ask":["Bash"]}}"#,
          "--disallowedTools",
          "Edit",
          "Write",
          "NotebookEdit",
          "WebFetch",
          "WebSearch",
          "Read",
          "MultiEdit",
          "NotebookRead",
          "Grep",
          "Glob",
          "LS",
          "--allowedTools",
        ]
        .into_iter()
        .chain(READ_RULES)
        .collect::<Vec<_>>()
      )
    );
  }
}

/// The listed commands run; any other asks a person, which `dontAsk` would deny before asking.
#[test]
fn claude_invocation_read_only_runs_the_listed_commands_and_asks_for_others() {
  for platform in PLATFORMS {
    let permissions = enforced(
      RepositoryAccess::ReadOnly,
      allowed(&["git log", "rg"]),
      NetworkAccess::On,
      platform,
    );
    assert_eq!(
      args(&invocation(&permissions)),
      expected(
        true,
        &[
          "--tools",
          "Read,Glob,Grep,TodoWrite,Bash,WebFetch,WebSearch",
          "--permission-mode",
          "default",
          "--disallowedTools",
          "Edit",
          "Write",
          "NotebookEdit",
          "Read",
          "MultiEdit",
          "NotebookRead",
          "Grep",
          "Glob",
          "LS",
          "--allowedTools",
          "Bash(git log:*)",
          "Bash(rg:*)",
        ]
        .into_iter()
        .chain(READ_RULES)
        .collect::<Vec<_>>()
      )
    );
  }
}

#[test]
fn claude_invocation_write_allows_the_listed_commands_and_asks_for_others() {
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
      expected(
        true,
        &[
          "--tools",
          "Read,Glob,Grep,TodoWrite,Edit,Write,NotebookEdit,Bash",
          "--permission-mode",
          "default",
          "--disallowedTools",
          "WebFetch",
          "WebSearch",
          "Read",
          "Edit",
          "MultiEdit",
          "Write",
          "NotebookEdit",
          "NotebookRead",
          "Grep",
          "Glob",
          "LS",
          "--allowedTools",
          "Bash(git:*)",
          "Bash(pnpm test:*)",
          "Edit(//repo/.orchester/worktrees/run/**)",
        ]
        .into_iter()
        .chain(WRITE_RULES)
        .collect::<Vec<_>>()
      )
    );
  }
}

/// `acceptEdits` would run `rm`, `sed` and the other file commands in any form, so a step with
/// allowed commands edits by rule instead.
#[test]
fn claude_invocation_edits_by_rule_whenever_commands_are_listed() {
  for platform in PLATFORMS {
    let repository = |repository, commands| {
      enforced(repository, commands, NetworkAccess::On, platform).mechanism(PermissionSetting::Repository)
    };
    for listed in [&["git", "sed -n"][..], &["rm -f build/x"], &["git", "pnpm test"]] {
      assert_eq!(
        repository(RepositoryAccess::Write, allowed(listed)),
        Mechanism::ClaudeEditRules,
        "{listed:?}"
      );
    }
    for commands in [allowed(&[]), CommandPermission::AskBefore] {
      assert_eq!(
        repository(RepositoryAccess::Write, commands),
        Mechanism::ClaudeAcceptEdits
      );
    }
    assert_eq!(
      repository(RepositoryAccess::ReadOnly, allowed(&["sed -n"])),
      Mechanism::ClaudeEditToolsRemoved
    );
  }
}

#[test]
fn claude_invocation_write_runs_listed_file_commands_in_their_listed_forms_only() {
  for platform in PLATFORMS {
    let permissions = enforced(
      RepositoryAccess::Write,
      allowed(&["sed -n", "rm -f build/x"]),
      NetworkAccess::On,
      platform,
    );
    // No deny rule: every other form asks a person, as any unlisted command does.
    assert_eq!(
      args(&invocation(&permissions)),
      expected(
        true,
        &[
          "--tools",
          "Read,Glob,Grep,TodoWrite,Edit,Write,NotebookEdit,Bash,WebFetch,WebSearch",
          "--permission-mode",
          "default",
          "--allowedTools",
          "Bash(sed -n:*)",
          "Bash(rm -f build/x:*)",
          "Edit(//repo/.orchester/worktrees/run/**)",
        ]
        .into_iter()
        .chain(WRITE_RULES)
        .chain(["--disallowedTools"])
        .chain(OWN_FILE_TOOLS)
        .collect::<Vec<_>>()
      )
    );
  }
}

#[test]
fn claude_invocation_refuses_edits_by_rule_on_a_network_share() {
  let share = Path::new(r"\\server\share\repo\.orchester\worktrees\run");
  let start = |permissions: &EnforcedPermissions, worktree, roots: &[std::path::PathBuf]| {
    ClaudeInvocation {
      worktree,
      writable_roots: roots,
      ..invocation(permissions)
    }
    .command("Do it", Vec::new(), &CommandEnvironment::default())
    .err()
  };
  let by_rule = enforced(
    RepositoryAccess::Write,
    allowed(&["rm -f build/x"]),
    NetworkAccess::On,
    Platform::Windows,
  );
  // No `Edit` rule can name a network share, so every edit there would be denied unseen.
  assert_eq!(
    start(&by_rule, share, &[]),
    Some(InvocationError::NoEditRule(share.to_path_buf()))
  );
  let accepting = enforced(
    RepositoryAccess::Write,
    allowed(&[]),
    NetworkAccess::On,
    Platform::Windows,
  );
  assert_eq!(start(&accepting, share, &[]), None, "acceptEdits needs no rule");

  let mut with_folder = AgentPermissions::new(RepositoryAccess::Write, allowed(&["rm -f build/x"]), NetworkAccess::On);
  with_folder.set_writable_folders(serde_json::from_str(r#"["\\\\server\\share\\cache"]"#).expect("a listed folder"));
  let with_folder = with_folder
    .enforced_on(AgentProvider::ClaudeCodeCli, Platform::Windows)
    .expect("enforceable");
  let cache = std::path::PathBuf::from(r"\\server\share\cache");
  assert_eq!(
    start(
      &with_folder,
      Path::new("/repo/.orchester/worktrees/run"),
      std::slice::from_ref(&cache)
    ),
    Some(InvocationError::NoEditRule(cache))
  );
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
  assert!(args.contains(&"default".to_owned()), "permissions are given again");
  assert!(args.contains(&PROMPT_TOOL.to_owned()), "so is the permission prompt");
  assert!(matches!(ClaudeSession::continuing(None), ClaudeSession::New(_)));
  assert_eq!(ClaudeSession::continuing(Some("s")), ClaudeSession::Resume("s"));
}

#[test]
fn claude_invocation_passes_a_full_model_id_to_new_and_resumed_sessions() {
  let permissions = enforced(
    RepositoryAccess::Write,
    allowed(&["git"]),
    NetworkAccess::On,
    Platform::current(),
  );
  for session in [ClaudeSession::New(SESSION), ClaudeSession::Resume("5f0c2a1e")] {
    let mut current = invocation(&permissions);
    current.model = "claude-opus-5-5";
    current.effort = Some("max");
    current.session = session;
    let args = args(&current);
    assert_eq!(&args[7..11], ["--model", "claude-opus-5-5", "--effort", "max"]);
    assert_eq!(args.iter().filter(|arg| *arg == "--model").count(), 1);
  }
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
    .command("Do it", environment, &CommandEnvironment::default())
    .expect("unicode worktree");
  assert_eq!(command.program(), Path::new("/bin/claude"));
  assert_eq!(command.current_dir(), Some(Path::new("/repo/.orchester/worktrees/run")));
  // The prompt is the first stream-json user message; the input stays open for messages sent now.
  assert_eq!(command.stdin(), Some(input_line("Do it").as_str()));
  assert!(command.interactive_stdin());
  let line: serde_json::Value = serde_json::from_str(&input_line("Do it")).expect("JSON");
  assert_eq!(line["type"], "user");
  assert_eq!(line["message"]["content"][0]["text"], "Do it");
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
    .command("Do it", environment, &project)
    .expect("unicode worktree");
  assert_eq!(
    command.env(),
    [
      ("PATH".to_owned(), "/usr/bin".to_owned()),
      ("CARGO_TARGET_DIR".to_owned(), "/repo/.orchester/target".to_owned()),
    ]
  );
}
