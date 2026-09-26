//! Claude Code (`claude -p`) arguments that enforce permissions (`.tmp/cli-probe/REPORT.md`,
//! "Recommended argv"). Prepared for the Claude invocation; Claude runs are still refused by
//! preflight, and the denial behaviour of these flags is unverified.
//!
//! Never used: `--dangerously-skip-permissions`, `--permission-mode bypassPermissions`.

use super::{
  AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess,
  capability::{EnforcedPermissions, PermissionProblem},
};
use crate::connection::{AgentProvider, invocation::Platform};

/// The permission arguments of a Claude Code invocation on `platform`, or why Claude Code cannot
/// enforce `permissions` there.
pub fn permission_args(permissions: &AgentPermissions, platform: Platform) -> Result<Vec<String>, PermissionProblem> {
  let enforced = permissions.enforced_on(AgentProvider::ClaudeCodeCli, platform)?;
  Ok(args(&enforced))
}

fn args(enforced: &EnforcedPermissions) -> Vec<String> {
  let permissions = enforced.permissions();
  let mut disallowed: Vec<&str> = Vec::new();
  let mut args: Vec<String> = vec!["--permission-mode".into()];
  match permissions.repository() {
    RepositoryAccess::ReadOnly => {
      args.push("dontAsk".into());
      disallowed.extend(["Edit", "Write", "NotebookEdit"]);
    }
    RepositoryAccess::Write => args.push("acceptEdits".into()),
  }
  if permissions.network() == NetworkAccess::Off {
    disallowed.extend(["WebFetch", "WebSearch"]);
  }
  if !disallowed.is_empty() {
    args.push("--disallowedTools".into());
    args.extend(disallowed.into_iter().map(str::to_owned));
  }
  // Enforced for Claude Code, so the commands are a list: Bash runs only what a rule allows.
  if let CommandPermission::Allowed(commands) = permissions.commands()
    && !commands.is_empty()
  {
    args.push("--allowedTools".into());
    args.extend(commands.entries().iter().map(|entry| format!("Bash({entry}:*)")));
  }
  args
}
