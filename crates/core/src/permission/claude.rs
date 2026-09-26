//! Claude Code (`claude -p`) arguments that enforce permissions (`.tmp/cli-probe/REPORT.md`,
//! "Recommended argv"), for [`crate::connection::invocation::ClaudeInvocation`].
//!
//! - `--tools` offers only the built-in tools the permissions need: reading and searching always, the edit tools for
//!   *Edit files in the task worktree*, `Bash` only when commands are allowed, the web tools only with network access.
//!   Scheduling, remote triggers, worktree switching and sub-agents are never offered.
//! - Read-only runs in `dontAsk`: whatever no rule allows is denied. Writing runs in `acceptEdits`: edits in the
//!   working directory are accepted, and in print mode nothing else is asked, so a command no rule allows is denied and
//!   the CLI reports the denial.
//! - Allowed commands become `--allowedTools "Bash(<prefix>:*)"` prefix rules. Without any, `Bash` is removed (the
//!   probe: it can change files, so a read-only step must not have it).
//! - Network off cannot be enforced (commands keep network access, and Claude Code's own sandbox is not supported on
//!   Windows), so network stays on; when off was chosen the web tools are still removed.
//! - The disallowed list repeats what `--tools` leaves out that the permissions deny, so a tool list that grows in a
//!   later CLI version does not widen them.
//!
//! Never used: `--dangerously-skip-permissions`, `--permission-mode bypassPermissions`. The denial
//! behaviour of these flags with a model turn is unverified: the probe's CLI was not signed in.

use super::{CommandPermission, NetworkAccess, RepositoryAccess, capability::EnforcedPermissions};

const READ_TOOLS: [&str; 4] = ["Read", "Glob", "Grep", "TodoWrite"];
const EDIT_TOOLS: [&str; 3] = ["Edit", "Write", "NotebookEdit"];
const WEB_TOOLS: [&str; 2] = ["WebFetch", "WebSearch"];
const COMMAND_TOOL: &str = "Bash";

/// The permission arguments of a Claude Code invocation that runs with `enforced`.
pub fn permission_args(enforced: &EnforcedPermissions) -> Vec<String> {
  let permissions = enforced.permissions();
  let commands = match permissions.commands() {
    CommandPermission::Allowed(commands) => commands.entries(),
    // Refused by `enforced_on` for Claude Code; nothing runs without a rule.
    CommandPermission::AnyInWorktree | CommandPermission::AskBefore => &[],
  };
  let network = permissions.network() == NetworkAccess::On && enforced.network_stays_on().is_none();
  let mut offered: Vec<&str> = READ_TOOLS.to_vec();
  let mut denied: Vec<&str> = Vec::new();
  let mode = match permissions.repository() {
    RepositoryAccess::ReadOnly => {
      denied.extend(EDIT_TOOLS);
      "dontAsk"
    }
    RepositoryAccess::Write => {
      offered.extend(EDIT_TOOLS);
      "acceptEdits"
    }
  };
  if commands.is_empty() {
    denied.push(COMMAND_TOOL);
  } else {
    offered.push(COMMAND_TOOL);
  }
  if network {
    offered.extend(WEB_TOOLS);
  } else {
    denied.extend(WEB_TOOLS);
  }
  let mut args = vec![
    "--tools".to_owned(),
    offered.join(","),
    "--permission-mode".to_owned(),
    mode.to_owned(),
  ];
  if !denied.is_empty() {
    args.push("--disallowedTools".to_owned());
    args.extend(denied.into_iter().map(str::to_owned));
  }
  if !commands.is_empty() {
    args.push("--allowedTools".to_owned());
    args.extend(commands.iter().map(|entry| format!("{COMMAND_TOOL}({entry}:*)")));
  }
  args
}
