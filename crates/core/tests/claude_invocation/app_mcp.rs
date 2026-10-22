//! A Claude Code agent never reaches the app's own MCP server `orchester`, which the person registers in their own
//! Claude Code (`docs/design/app-mcp-connection.md`, Security › Agents in runs): it loads no MCP server or settings
//! file of the user's, and starts without the server's token or a QA copy's discovery folder.

use std::{ffi::OsString, path::Path};

use orchester_core::{
  connection::invocation::{ClaudeInvocation, ClaudeSession},
  permission::{NetworkAccess, RepositoryAccess},
  resource::CommandEnvironment,
};

use crate::{APPROVALS, PLATFORMS, allowed, args, enforced, invocation};

/// The MCP configuration files an invocation passes, by `--mcp-config`.
fn mcp_configs(args: &[String]) -> Vec<&str> {
  args
    .windows(2)
    .filter(|pair| pair[0] == "--mcp-config")
    .map(|pair| pair[1].as_str())
    .collect()
}

/// `--strict-mcp-config` keeps a user-scope server of `~/.claude.json` out, `--setting-sources ""` every settings
/// file: the only MCP configuration is the invocation's own, on new and resumed sessions alike.
#[test]
fn app_mcp_claude_run_loads_no_user_mcp_server() {
  for platform in PLATFORMS {
    for repository in [RepositoryAccess::ReadOnly, RepositoryAccess::Write] {
      for commands in [allowed(&[]), allowed(&["git"])] {
        let permissions = enforced(repository, commands, NetworkAccess::On, platform);
        let new = invocation(&permissions);
        let resumed = ClaudeInvocation {
          session: ClaudeSession::Resume("5f0c2a1e"),
          ..invocation(&permissions)
        };
        for run in [new, resumed] {
          let args = args(&run);
          assert_eq!(
            args.iter().filter(|arg| *arg == "--strict-mcp-config").count(),
            1,
            "{args:?}"
          );
          let sources = args
            .iter()
            .position(|arg| arg == "--setting-sources")
            .expect("setting sources");
          assert_eq!(args[sources + 1], "", "{args:?}");
          assert!(mcp_configs(&args).iter().all(|config| *config == APPROVALS), "{args:?}");
          assert!(!args.iter().any(|arg| arg.contains("mcp__orchester__")), "{args:?}");
        }
      }
    }
  }
}

/// The token of the app's MCP server and the variable naming a QA copy's discovery folder are withheld from the
/// agent, in any spelling Windows would match.
#[test]
fn app_mcp_token_variable_is_not_passed_to_claude() {
  let environment = [
    ("PATH", "/usr/bin"),
    ("ORCHESTER_APP_MCP_TOKEN", "app-token"),
    ("orchester_app_mcp_token", "app-token"),
    ("ORCHESTER_APP_MCP_DISCOVERY", "/repo/.tmp/qa/task/mcp"),
    ("CLAUDE_CONFIG_DIR", "/home/me/.claude"),
  ];
  for platform in PLATFORMS {
    let permissions = enforced(RepositoryAccess::Write, allowed(&["git"]), NetworkAccess::On, platform);
    let command = invocation(&permissions)
      .command(
        ("Do it", &[]),
        environment.map(|(name, value)| (OsString::from(name), OsString::from(value))),
        &CommandEnvironment::default(),
      )
      .expect("unicode worktree");
    assert_eq!(command.program(), Path::new("/bin/claude"));
    let names: Vec<&str> = command.env().iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["PATH", "CLAUDE_CONFIG_DIR"], "{platform:?}");
    assert!(
      !command.env().iter().any(|(_, value)| value == "app-token"),
      "{platform:?}"
    );
  }
}
