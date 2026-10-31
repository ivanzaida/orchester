//! Network access limited to a host list (ORC-450, `docs/evidence/orc-450-network-hosts-2026-10-30.md`): each agent
//! CLI gets the list in the form it enforces, and a CLI that can't enforce it refuses the step before it starts,
//! naming the CLI. Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test network_hosts
//! network_hosts`.

mod support;

use std::path::Path;

use orchester_core::{
  connection::{
    AgentProvider,
    invocation::{CodexAgentFiles, CodexInvocation, Platform},
  },
  id::IdempotencyKey,
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, HostPattern, NetworkAccess, NetworkHosts, RepositoryAccess,
    capability::{Mechanism, PermissionOption, PermissionProblem, PermissionSetting, Unavailable, support},
  },
  resource::StepKey,
  run::PreflightProblem,
};
use orchester_ipc::Rejection;

use self::support::{Setup, project, scheduler};

fn hosts(patterns: &[&str]) -> NetworkAccess {
  let patterns = patterns
    .iter()
    .map(|pattern| HostPattern::new(pattern).expect("a host pattern"))
    .collect();
  NetworkAccess::Hosts(NetworkHosts::new(patterns).expect("a host list"))
}

fn permissions(repository: RepositoryAccess, commands: CommandPermission) -> AgentPermissions {
  AgentPermissions::new(repository, commands, hosts(&["*.example.org", "api.example.com"]))
}

/// The arguments Codex starts with for `access` and `network` on macOS, where its sandbox keeps commands off the
/// network.
fn codex_args(access: RepositoryAccess, network: NetworkAccess) -> Vec<String> {
  CodexInvocation {
    executable: Path::new("/bin/codex"),
    repository: Path::new("/repo"),
    access,
    network,
    writable_roots: &[],
    worktree: Path::new("/repo/.orchester/worktrees/run"),
    platform: Platform::MacOs,
    mcp: None,
    agent_files: CodexAgentFiles::Loaded,
  }
  .args()
  .expect("unicode paths")
}

/// The value of each `-c` override that starts with `key`.
fn overrides<'a>(args: &'a [String], key: &str) -> Vec<&'a str> {
  args
    .windows(2)
    .filter(|pair| pair[0] == "-c" && pair[1].starts_with(key))
    .map(|pair| pair[1].as_str())
    .collect()
}

#[test]
fn network_hosts_codex_keeps_commands_off_the_network_and_web_search_on_the_listed_domains() {
  let enforced = permissions(RepositoryAccess::Write, CommandPermission::AnyInWorktree)
    .enforced_on(AgentProvider::CodexCli, Platform::MacOs)
    .expect("Codex enforces a host list on macOS");
  assert_eq!(
    enforced.mechanism(PermissionSetting::Network),
    Mechanism::CodexSandboxNetworkHosts
  );
  assert_eq!(
    *enforced.permissions().network(),
    hosts(&["*.example.org", "api.example.com"]),
    "the list is run as it is, never as network on"
  );
  let list = hosts(&["*.example.org", "api.example.com", "example.org"]);
  let write = codex_args(RepositoryAccess::Write, list.clone());
  assert_eq!(
    overrides(&write, "sandbox_workspace_write.network_access"),
    ["sandbox_workspace_write.network_access=false"]
  );
  let domains = [r#"tools.web_search.allowed_domains=["api.example.com","example.org"]"#];
  assert_eq!(overrides(&write, "tools.web_search"), domains, "{write:?}");
  // The read-only sandbox has no network setting: its commands never reach the network.
  let read = codex_args(RepositoryAccess::ReadOnly, list);
  assert!(overrides(&read, "sandbox_workspace_write.network_access").is_empty());
  assert_eq!(overrides(&read, "tools.web_search"), domains, "{read:?}");
  for network in [NetworkAccess::Off, NetworkAccess::On] {
    let args = codex_args(RepositoryAccess::Write, network);
    assert!(overrides(&args, "tools.web_search").is_empty(), "{args:?}");
  }
}

#[test]
fn network_hosts_are_refused_where_the_cli_cannot_limit_commands_to_them() {
  let listed = CommandPermission::Allowed(AllowedCommands::default());
  let refused = |provider, platform, commands: &CommandPermission| {
    permissions(RepositoryAccess::Write, commands.clone()).enforced_on(provider, platform)
  };
  let no_hosts = Err(PermissionProblem::NotEnforceable {
    option: PermissionOption::NetworkHosts,
    reason: Unavailable::CannotLimitHosts,
  });
  assert_eq!(
    refused(
      AgentProvider::CodexCli,
      Platform::Windows,
      &CommandPermission::AnyInWorktree
    ),
    no_hosts,
    "Codex's Windows sandbox keeps no command off the network"
  );
  for platform in [Platform::Windows, Platform::MacOs] {
    assert_eq!(
      refused(AgentProvider::ClaudeCodeCli, platform, &listed),
      no_hosts,
      "Claude Code's commands reach any host on {platform:?}"
    );
    assert_eq!(
      refused(AgentProvider::AnthropicApi, platform, &listed),
      no_hosts,
      "{platform:?}"
    );
  }
}

/// A step whose profile limits the network to a host list on a CLI that can't enforce it here never starts: the
/// run is refused before it starts, naming the step and the CLI.
#[tokio::test(flavor = "multi_thread")]
async fn network_hosts_refuse_the_step_before_it_starts_naming_the_cli() {
  for (provider, commands) in [
    (AgentProvider::ClaudeCodeCli, CommandPermission::AskBefore),
    (AgentProvider::CodexCli, CommandPermission::AnyInWorktree),
  ] {
    if support(provider, Platform::current(), PermissionOption::NetworkHosts).is_enforced() {
      // Codex on macOS runs the list (see above); this computer runs it rather than refusing it.
      continue;
    }
    let project = project(&Setup {
      provider,
      permissions: Some(permissions(RepositoryAccess::Write, commands)),
      ..Setup::new("Append one line.")
    });
    let (scheduler, _events) = scheduler(project.repo.path());
    assert_eq!(
      scheduler.start_run(IdempotencyKey::generate(), project.task).await,
      Err(Rejection::Preflight {
        problems: vec![PreflightProblem::NetworkHostsNotEnforceable {
          step: StepKey::new("implement").expect("key"),
          cli: provider,
        }]
      }),
      "{provider:?}"
    );
  }
}
