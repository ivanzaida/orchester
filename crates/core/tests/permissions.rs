//! Agent permissions: validated values, the capability table, narrowing by steps, what publishing
//! and preflight refuse, and the CLI arguments they map to.

use std::path::Path;

use orchester_core::{
  connection::{
    AgentProvider,
    invocation::{CodexAgentFiles, CodexInvocation, Platform},
  },
  permission::{
    AgentPermissions, AllowedCommands, AllowedCommandsError, CommandPermission, CommandPrefix, CommandPrefixError,
    NetworkAccess, RepositoryAccess, StepCommands, StepNetwork, StepPermissions,
    capability::{
      Mechanism, PermissionOption as O, PermissionProblem, PermissionSetting, Support, Unavailable, support,
    },
    claude,
  },
};

const PLATFORMS: [Platform; 2] = [Platform::Windows, Platform::MacOs];

fn prefix(value: &str) -> CommandPrefix {
  CommandPrefix::new(value).expect("valid command")
}

fn allowed(values: &[&str]) -> CommandPermission {
  CommandPermission::Allowed(commands(values))
}

fn commands(values: &[&str]) -> AllowedCommands {
  AllowedCommands::new(values.iter().map(|value| prefix(value)).collect()).expect("valid list")
}

fn permissions(repository: RepositoryAccess, commands: CommandPermission, network: NetworkAccess) -> AgentPermissions {
  AgentPermissions::new(repository, commands, network)
}

#[test]
fn permissions_command_prefix_normalizes_spaces_and_rejects_chaining() {
  assert_eq!(prefix("  pnpm   test ").as_str(), "pnpm test");
  assert_eq!(CommandPrefix::new(" \t"), Err(CommandPrefixError::Blank));
  for (value, forbidden) in [
    ("git; rm", ';'),
    ("a && b", '&'),
    ("cat x | y", '|'),
    ("echo $(id)", '$'),
  ] {
    assert_eq!(
      CommandPrefix::new(value),
      Err(CommandPrefixError::ForbiddenCharacter(forbidden))
    );
  }
  assert_eq!(
    CommandPrefix::new("x\ny"),
    Ok(prefix("x y")),
    "line breaks are whitespace between words"
  );
  assert_eq!(
    CommandPrefix::new(&"a".repeat(CommandPrefix::MAX_CHARS + 1)),
    Err(CommandPrefixError::TooLong)
  );
}

#[test]
fn permissions_command_prefix_matches_whole_words() {
  assert!(prefix("pnpm test").covers(&prefix("pnpm test src/auth")));
  assert!(prefix("pnpm test").covers(&prefix("pnpm test")));
  assert!(!prefix("pnpm test").covers(&prefix("pnpm tester")));
  assert!(!prefix("pnpm test src").covers(&prefix("pnpm test")));
}

#[test]
fn permissions_allowed_commands_reject_duplicates_and_long_lists() {
  assert_eq!(
    AllowedCommands::new(vec![prefix("git"), prefix("git")]),
    Err(AllowedCommandsError::Duplicate(prefix("git")))
  );
  let many = (0..=AllowedCommands::MAX_ENTRIES)
    .map(|index| prefix(&format!("tool{index}")))
    .collect();
  assert_eq!(AllowedCommands::new(many), Err(AllowedCommandsError::TooMany));
}

#[test]
fn permissions_round_trip_and_revalidate_on_read() {
  let value = permissions(
    RepositoryAccess::Write,
    allowed(&["git", "pnpm test"]),
    NetworkAccess::Off,
  );
  let json = serde_json::to_string(&value).expect("serialize");
  assert_eq!(
    serde_json::from_str::<AgentPermissions>(&json).expect("deserialize"),
    value
  );
  let chained = json.replace("pnpm test", "pnpm test; curl x");
  assert!(serde_json::from_str::<AgentPermissions>(&chained).is_err());
  let duplicate = json.replace("\"pnpm test\"", "\"git\"");
  assert!(serde_json::from_str::<AgentPermissions>(&duplicate).is_err());
}

/// The whole capability table, row by row, so any change to it is deliberate.
#[test]
fn permissions_capability_table() {
  use Mechanism as M;
  let enforced = Support::Enforced;
  let no = Support::Unavailable;
  let codex = [
    (
      O::ReadOnly,
      enforced(M::CodexReadOnlySandbox),
      enforced(M::CodexReadOnlySandbox),
    ),
    (
      O::EditWorktree,
      enforced(M::CodexWorkspaceWriteSandbox),
      enforced(M::CodexWorkspaceWriteSandbox),
    ),
    (
      O::AskBeforeCommands,
      enforced(M::CodexCommandApprovals),
      enforced(M::CodexCommandApprovals),
    ),
    (
      O::AllowedCommands,
      no(Unavailable::CannotLimitCommands),
      no(Unavailable::CannotLimitCommands),
    ),
    (
      O::AnyCommand,
      enforced(M::CodexSandboxedCommands),
      enforced(M::CodexSandboxedCommands),
    ),
    (
      O::NetworkOff,
      no(Unavailable::CannotBlockNetwork),
      enforced(M::CodexSandboxNetworkOff),
    ),
    (
      O::NetworkOn,
      enforced(M::CodexSandboxNetworkOn),
      enforced(M::CodexSandboxNetworkOn),
    ),
  ];
  let claude = [
    (O::ReadOnly, enforced(M::ClaudeEditToolsRemoved)),
    (O::EditWorktree, enforced(M::ClaudeAcceptEdits)),
    (O::AskBeforeCommands, no(Unavailable::NeedsPermissionRequests)),
    (O::AllowedCommands, enforced(M::ClaudeAllowedTools)),
    (O::AnyCommand, no(Unavailable::CannotConfineCommands)),
    (O::NetworkOff, no(Unavailable::CannotBlockNetwork)),
    (O::NetworkOn, enforced(M::ClaudeWebTools)),
  ];
  for (option, windows, mac) in codex {
    assert_eq!(
      support(AgentProvider::CodexCli, Platform::Windows, option),
      windows,
      "{option:?}"
    );
    assert_eq!(
      support(AgentProvider::CodexCli, Platform::MacOs, option),
      mac,
      "{option:?}"
    );
  }
  for (option, expected) in claude {
    for platform in PLATFORMS {
      assert_eq!(
        support(AgentProvider::ClaudeCodeCli, platform, option),
        expected,
        "{option:?}"
      );
    }
  }
}

#[test]
fn permissions_codex_refuses_a_command_list_and_keeps_network_on_where_it_cannot_block_it() {
  let listed = permissions(RepositoryAccess::Write, allowed(&["git"]), NetworkAccess::On);
  for platform in PLATFORMS {
    assert_eq!(
      listed.enforced_on(AgentProvider::CodexCli, platform),
      Err(PermissionProblem::NotEnforceable {
        option: O::AllowedCommands,
        reason: Unavailable::CannotLimitCommands,
      })
    );
  }
  let offline = permissions(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::Off,
  );
  let windows = offline
    .enforced_on(AgentProvider::CodexCli, Platform::Windows)
    .expect("enforceable");
  assert_eq!(windows.permissions().network(), NetworkAccess::On);
  assert_eq!(windows.network_stays_on(), Some(Unavailable::CannotBlockNetwork));
  assert_eq!(
    windows.mechanism(PermissionSetting::Repository),
    Mechanism::CodexWorkspaceWriteSandbox
  );
  let mac = offline
    .enforced_on(AgentProvider::CodexCli, Platform::MacOs)
    .expect("enforceable");
  assert_eq!(mac.permissions(), &offline);
  assert_eq!(mac.network_stays_on(), None);
}

#[test]
fn permissions_network_that_stays_on_names_how_it_runs() {
  let offline = permissions(RepositoryAccess::ReadOnly, allowed(&["git"]), NetworkAccess::Off);
  let online = permissions(RepositoryAccess::ReadOnly, allowed(&["git"]), NetworkAccess::On);
  for platform in PLATFORMS {
    // Claude Code still removes its web tools when off was chosen; only commands keep network.
    let stays_on = offline
      .enforced_on(AgentProvider::ClaudeCodeCli, platform)
      .expect("enforceable");
    assert_eq!(
      stays_on.mechanism(PermissionSetting::Network),
      Mechanism::ClaudeWebToolsRemoved
    );
    let on = online
      .enforced_on(AgentProvider::ClaudeCodeCli, platform)
      .expect("enforceable");
    assert_eq!(on.mechanism(PermissionSetting::Network), Mechanism::ClaudeWebTools);
  }
  let codex = permissions(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::Off,
  )
  .enforced_on(AgentProvider::CodexCli, Platform::Windows)
  .expect("enforceable");
  assert_eq!(
    codex.mechanism(PermissionSetting::Network),
    Mechanism::CodexSandboxNetworkOn
  );
}

#[test]
fn permissions_effective_network_is_on_where_off_cannot_be_enforced() {
  let offline = permissions(RepositoryAccess::ReadOnly, allowed(&["git"]), NetworkAccess::Off);
  let online = permissions(RepositoryAccess::ReadOnly, allowed(&["git"]), NetworkAccess::On);
  for platform in PLATFORMS {
    assert_eq!(
      offline.effective_network(AgentProvider::ClaudeCodeCli, platform),
      NetworkAccess::On
    );
    assert_eq!(
      online.effective_network(AgentProvider::ClaudeCodeCli, platform),
      NetworkAccess::On
    );
    let enforced = offline
      .enforced_on(AgentProvider::ClaudeCodeCli, platform)
      .expect("enforceable");
    assert_eq!(enforced.permissions().network(), NetworkAccess::On);
  }
  let codex = permissions(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::Off,
  );
  assert_eq!(
    codex.effective_network(AgentProvider::CodexCli, Platform::Windows),
    NetworkAccess::On
  );
  assert_eq!(
    codex.effective_network(AgentProvider::CodexCli, Platform::MacOs),
    NetworkAccess::Off
  );
  // Settings that cannot run leave the network's answer alone.
  let unconfined = permissions(
    RepositoryAccess::ReadOnly,
    CommandPermission::AnyInWorktree,
    NetworkAccess::Off,
  );
  assert!(
    unconfined
      .enforced_on(AgentProvider::ClaudeCodeCli, Platform::Windows)
      .is_err()
  );
  assert_eq!(
    unconfined.effective_network(AgentProvider::ClaudeCodeCli, Platform::Windows),
    NetworkAccess::On
  );
}

#[test]
fn permissions_claude_refuses_unconfined_commands() {
  let any = permissions(
    RepositoryAccess::ReadOnly,
    CommandPermission::AnyInWorktree,
    NetworkAccess::On,
  );
  assert_eq!(
    any.enforced_on(AgentProvider::ClaudeCodeCli, Platform::Windows),
    Err(PermissionProblem::NotEnforceable {
      option: O::AnyCommand,
      reason: Unavailable::CannotConfineCommands,
    })
  );
}

#[test]
fn permissions_fitting_to_a_connection_resets_to_the_most_restrictive_enforceable_option() {
  let listed = permissions(RepositoryAccess::Write, allowed(&["git"]), NetworkAccess::Off);
  let (codex, resets) = listed.fitted_to(AgentProvider::CodexCli, Platform::Windows);
  assert_eq!(
    codex,
    permissions(
      RepositoryAccess::Write,
      CommandPermission::AnyInWorktree,
      NetworkAccess::On
    )
  );
  assert_eq!(resets, [O::AllowedCommands, O::NetworkOff]);
  let (mac, resets) = listed.fitted_to(AgentProvider::CodexCli, Platform::MacOs);
  assert_eq!(mac.network(), NetworkAccess::Off);
  assert_eq!(resets, [O::AllowedCommands]);

  let (claude, resets) = codex.fitted_to(AgentProvider::ClaudeCodeCli, Platform::MacOs);
  assert_eq!(
    claude,
    permissions(RepositoryAccess::Write, allowed(&[]), NetworkAccess::On)
  );
  assert_eq!(resets, [O::AnyCommand]);
  assert_eq!(claude.fitted_to(AgentProvider::ClaudeCodeCli, Platform::MacOs).1, []);
}

#[test]
fn permissions_new_profiles_start_at_the_most_restrictive_enforceable_values() {
  assert_eq!(
    AgentPermissions::most_restrictive(AgentProvider::CodexCli, Platform::Windows),
    permissions(
      RepositoryAccess::ReadOnly,
      CommandPermission::AnyInWorktree,
      NetworkAccess::On
    )
  );
  assert_eq!(
    AgentPermissions::most_restrictive(AgentProvider::CodexCli, Platform::MacOs),
    permissions(
      RepositoryAccess::ReadOnly,
      CommandPermission::AnyInWorktree,
      NetworkAccess::Off
    )
  );
  assert_eq!(
    AgentPermissions::most_restrictive(AgentProvider::ClaudeCodeCli, Platform::Windows),
    permissions(RepositoryAccess::ReadOnly, allowed(&[]), NetworkAccess::On)
  );
}

#[test]
fn permissions_profiles_before_permissions_read_only_and_runnable_on_codex() {
  let before = AgentPermissions::before_permissions();
  assert_eq!(
    before,
    permissions(
      RepositoryAccess::ReadOnly,
      CommandPermission::AnyInWorktree,
      NetworkAccess::Off
    )
  );
  for platform in PLATFORMS {
    assert!(before.enforced_on(AgentProvider::CodexCli, platform).is_ok());
  }
}

#[test]
fn permissions_steps_only_narrow() {
  let profile = permissions(
    RepositoryAccess::Write,
    allowed(&["git", "pnpm test"]),
    NetworkAccess::On,
  );
  assert_eq!(
    profile.narrowed(RepositoryAccess::Write, &StepPermissions::default()),
    profile
  );
  let step = StepPermissions {
    commands: StepCommands::Only(commands(&["git log", "pnpm test src", "curl"])),
    network: StepNetwork::Off,
    ..StepPermissions::default()
  };
  assert_eq!(
    profile.narrowed(RepositoryAccess::ReadOnly, &step),
    permissions(
      RepositoryAccess::ReadOnly,
      allowed(&["git log", "pnpm test src"]),
      NetworkAccess::Off
    )
  );
  let read_only = permissions(
    RepositoryAccess::ReadOnly,
    CommandPermission::AnyInWorktree,
    NetworkAccess::Off,
  );
  let widening = StepPermissions {
    commands: StepCommands::Only(commands(&["curl"])),
    network: StepNetwork::AsProfile,
    ..StepPermissions::default()
  };
  assert_eq!(
    read_only.narrowed(RepositoryAccess::Write, &widening),
    permissions(RepositoryAccess::ReadOnly, allowed(&["curl"]), NetworkAccess::Off),
    "write access and network stay off; a list narrows any command"
  );
}

#[test]
fn permissions_step_narrowing_is_omitted_from_stored_steps_that_narrow_nothing() {
  assert!(StepPermissions::default().is_as_profile());
  let json = serde_json::to_string(&StepPermissions {
    commands: StepCommands::Only(commands(&["git"])),
    network: StepNetwork::Off,
    ..StepPermissions::default()
  })
  .expect("serialize");
  assert_eq!(
    json,
    r#"{"commands":{"kind":"only","commands":["git"]},"network":"off"}"#
  );
}

fn codex_args(permissions: &AgentPermissions, platform: Platform) -> Vec<String> {
  let enforced = permissions
    .enforced_on(AgentProvider::CodexCli, platform)
    .expect("enforceable on Codex");
  CodexInvocation {
    executable: Path::new("/bin/codex"),
    model: "gpt-5.5",
    reasoning_effort: None,
    access: enforced.permissions().repository(),
    network: enforced.permissions().network(),
    writable_roots: &[],
    worktree: Path::new("/w"),
    platform,
    resume: None,
    output_schema: None,
    mcp: None,
    agent_files: CodexAgentFiles::Loaded,
  }
  .args()
  .into_iter()
  .map(|arg| arg.into_string().expect("unicode"))
  .collect()
}

#[test]
fn permissions_map_to_codex_sandbox_arguments_per_platform() {
  let tail = |args: &[String]| args[args.iter().position(|arg| arg == "--sandbox").expect("sandbox")..].to_vec();
  let read_only = permissions(
    RepositoryAccess::ReadOnly,
    CommandPermission::AnyInWorktree,
    NetworkAccess::Off,
  );
  for platform in PLATFORMS {
    assert_eq!(
      tail(&codex_args(&read_only, platform)),
      ["--sandbox", "read-only", "-C", "/w", "-"]
    );
  }
  let offline = permissions(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::Off,
  );
  assert_eq!(
    tail(&codex_args(&offline, Platform::Windows)),
    [
      "--sandbox",
      "workspace-write",
      "-c",
      "windows.sandbox=elevated",
      "-c",
      "sandbox_workspace_write.network_access=true",
      "-C",
      "/w",
      "-"
    ],
    "network off cannot be enforced on Windows, so network stays on"
  );
  assert_eq!(
    tail(&codex_args(&offline, Platform::MacOs)),
    ["--sandbox", "workspace-write", "-C", "/w", "-"]
  );
  let online = permissions(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::On,
  );
  assert_eq!(
    tail(&codex_args(&online, Platform::MacOs)),
    [
      "--sandbox",
      "workspace-write",
      "-c",
      "sandbox_workspace_write.network_access=true",
      "-C",
      "/w",
      "-"
    ]
  );
}

#[test]
fn permissions_map_to_claude_arguments_without_bypass_modes() {
  let claude = |permissions: &AgentPermissions, platform| {
    permissions
      .enforced_on(AgentProvider::ClaudeCodeCli, platform)
      .map(|enforced| claude::permission_args(&enforced, &[]))
  };
  let read_only = permissions(
    RepositoryAccess::ReadOnly,
    allowed(&["git log", "rg"]),
    NetworkAccess::On,
  );
  assert_eq!(
    claude(&read_only, Platform::Windows).expect("enforceable"),
    [
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
      "Bash(rg:*)"
    ]
  );
  // Network off stays on for commands; without commands, Bash is not offered at all.
  let edit = permissions(RepositoryAccess::Write, allowed(&[]), NetworkAccess::Off);
  let args = claude(&edit, Platform::MacOs).expect("enforceable, network stays on");
  assert_eq!(
    args,
    [
      "--tools",
      "Read,Glob,Grep,TodoWrite,Edit,Write,NotebookEdit",
      "--permission-mode",
      "acceptEdits",
      "--disallowedTools",
      "Bash",
      "WebFetch",
      "WebSearch"
    ]
  );
  let any = permissions(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::On,
  );
  assert!(claude(&any, Platform::MacOs).is_err());
  for args in [claude(&read_only, Platform::MacOs).expect("enforceable"), args] {
    assert!(
      args
        .iter()
        .all(|arg| !arg.contains("dangerously") && arg != "bypassPermissions")
    );
  }
}
