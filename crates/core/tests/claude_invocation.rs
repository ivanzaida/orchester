//! How agent steps start the Claude Code CLI: its arguments per effective permissions and session,
//! and the environment it sees. Claude Code enforces the same options on Windows and macOS (its own
//! sandbox, the only platform-specific part, is not used), so every expectation is checked for both.

use std::{collections::BTreeMap, ffi::OsString, path::Path};

use orchester_core::{
  connection::{
    AgentProvider,
    invocation::{ClaudeInvocation, ClaudeSession, InvocationError, Platform, result_schema},
  },
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
    capability::{EnforcedPermissions, Mechanism, PermissionSetting},
  },
  resource::{CommandEnvironment, VariableName},
  run::STEP_RESULT_SCHEMA,
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
    writable_roots: &[],
    worktree: Path::new("/repo/.orchester/worktrees/run"),
    session: ClaudeSession::New(SESSION),
    mcp: None,
    skills: None,
    code_index: None,
    approvals: None,
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

/// [`BASE`], the result schema, `permissions`, then the new session.
fn expected(permissions: &[&str]) -> Vec<String> {
  let schema = result_schema();
  BASE
    .iter()
    .chain(&["--json-schema", schema.as_str()])
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
        // What `acceptEdits` would run without a rule.
        "Bash(mkdir:*)",
        "Bash(touch:*)",
        "Bash(rm:*)",
        "Bash(rmdir:*)",
        "Bash(mv:*)",
        "Bash(cp:*)",
        "Bash(sed:*)",
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
fn claude_invocation_edits_by_rule_when_a_listed_command_is_one_accept_edits_runs_unlisted() {
  for platform in PLATFORMS {
    let repository = |repository, commands| {
      enforced(repository, commands, NetworkAccess::On, platform).mechanism(PermissionSetting::Repository)
    };
    for listed in [
      &["git", "sed -n"][..],
      &["rm -f build/x"],
      &["git rm"],
      // A deny rule would match these after stripping `xargs`, a wrapper or an assignment.
      &["xargs mkdir"],
      &["timeout 60 rm -f build/x"],
      &["LANG=C sed -n"],
    ] {
      assert_eq!(
        repository(RepositoryAccess::Write, allowed(listed)),
        Mechanism::ClaudeEditRules,
        "{listed:?}"
      );
    }
    for commands in [
      allowed(&["git", "pnpm test"]),
      allowed(&[]),
      CommandPermission::AskBefore,
    ] {
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
fn claude_invocation_write_with_a_listed_file_command_edits_by_rule_without_accept_edits() {
  for platform in PLATFORMS {
    let permissions = enforced(
      RepositoryAccess::Write,
      allowed(&["sed -n", "rm -f build/x"]),
      NetworkAccess::On,
      platform,
    );
    // A `Bash(sed:*)` or `Bash(rm:*)` deny rule would deny the listed forms as well, and
    // `acceptEdits` would run every other form: the step edits by rule instead.
    assert_eq!(
      args(&invocation(&permissions)),
      expected(&[
        "--tools",
        "Read,Glob,Grep,TodoWrite,Edit,Write,NotebookEdit,Bash,WebFetch,WebSearch",
        "--permission-mode",
        "dontAsk",
        "--allowedTools",
        "Bash(sed -n:*)",
        "Bash(rm -f build/x:*)",
        "Edit(//repo/.orchester/worktrees/run/**)",
      ])
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
    .command("Do it".into(), Vec::new(), &CommandEnvironment::default())
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
    allowed(&["git"]),
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
  assert!(args.contains(&"acceptEdits".to_owned()), "permissions are given again");
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
    assert_eq!(&args[4..8], ["--model", "claude-opus-5-5", "--effort", "max"]);
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

#[test]
fn claude_invocation_reads_the_code_index_but_may_not_edit_it() {
  let index = [
    (
      Path::new("/repo/.orchester/index/4d2b8e0"),
      "Edit(//repo/.orchester/index/4d2b8e0/**)",
    ),
    (
      Path::new(r"C:\repo\.orchester\index\4d2b8e0"),
      "Edit(//c/repo/.orchester/index/4d2b8e0/**)",
    ),
  ];
  for (folder, rule) in index {
    let writing = enforced(
      RepositoryAccess::Write,
      allowed(&["git"]),
      NetworkAccess::On,
      Platform::Windows,
    );
    let given = ClaudeInvocation {
      code_index: Some(folder),
      ..invocation(&writing)
    };
    let given_args = args(&given);
    let added = given_args
      .windows(2)
      .any(|pair| pair[0] == "--add-dir" && pair[1] == folder.to_string_lossy());
    assert!(added, "{given_args:?}");
    assert_eq!(
      given_args
        .iter()
        .filter(|arg| arg.as_str() == "--disallowedTools")
        .count(),
      1,
      "{given_args:?}"
    );
    assert!(given_args.iter().any(|arg| arg == rule), "{given_args:?}");

    let reading = enforced(
      RepositoryAccess::ReadOnly,
      allowed(&["git"]),
      NetworkAccess::On,
      Platform::Windows,
    );
    let read_only = ClaudeInvocation {
      code_index: Some(folder),
      ..invocation(&reading)
    };
    assert!(
      !args(&read_only).iter().any(|arg| arg.starts_with("Edit(")),
      "every edit tool is denied already"
    );
  }
}

#[test]
fn claude_invocation_gets_no_edit_rule_for_a_code_index_on_a_network_share() {
  // How Claude Code writes a network path in its patterns is not documented: the index's files
  // are read-only instead.
  for folder in [
    r"\\server\share\repo\.orchester\index\4d2b8e0",
    r"\\?\UNC\server\share\repo\.orchester\index\4d2b8e0",
  ] {
    let writing = enforced(
      RepositoryAccess::Write,
      allowed(&["git"]),
      NetworkAccess::On,
      Platform::Windows,
    );
    let given = ClaudeInvocation {
      code_index: Some(Path::new(folder)),
      ..invocation(&writing)
    };
    let given_args = args(&given);
    assert!(!given_args.iter().any(|arg| arg.starts_with("Edit(")), "{given_args:?}");
  }
}
