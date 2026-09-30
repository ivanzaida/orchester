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
//! - The tools of the MCP servers the invocation gives the agent are allowed with one `mcp__<server>` rule per server;
//!   without a rule, print mode denies them. The servers themselves run outside these permissions.
//! - The disallowed list repeats what `--tools` leaves out that the permissions deny, so a tool list that grows in a
//!   later CLI version does not widen them.
//! - A folder the agent may read but must not change (the shared code index, which `--add-dir` makes a working
//!   directory, where `acceptEdits` accepts edits) gets an `Edit(//<folder>/**)` deny rule, which covers every built-in
//!   edit tool. Claude Code writes rule paths the POSIX way: `C:\x` is `//c/x`. Whether a model turn is denied is
//!   unverified; the index files are read-only on disk as well.
//!
//! Never used: `--dangerously-skip-permissions`, `--permission-mode bypassPermissions`. The denial
//! behaviour of these flags with a model turn is unverified: the probe's CLI was not signed in.

use std::path::Path;

use super::{CommandPermission, NetworkAccess, RepositoryAccess, capability::EnforcedPermissions};
use crate::connection::mcp::McpServerName;

const READ_TOOLS: [&str; 4] = ["Read", "Glob", "Grep", "TodoWrite"];
const EDIT_TOOLS: [&str; 3] = ["Edit", "Write", "NotebookEdit"];
const WEB_TOOLS: [&str; 2] = ["WebFetch", "WebSearch"];
const COMMAND_TOOL: &str = "Bash";

/// The permission arguments of a Claude Code invocation that runs with `enforced`, gives the agent the MCP
/// servers `mcp` and must not change the files under the folders `protected`.
pub fn permission_args(enforced: &EnforcedPermissions, mcp: &[McpServerName], protected: &[&Path]) -> Vec<String> {
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
  let protected_rules: Vec<String> = match permissions.repository() {
    // Every edit tool is denied already.
    RepositoryAccess::ReadOnly => Vec::new(),
    RepositoryAccess::Write => protected.iter().filter_map(|folder| edit_rule(folder)).collect(),
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
  if !denied.is_empty() || !protected_rules.is_empty() {
    args.push("--disallowedTools".to_owned());
    args.extend(denied.into_iter().map(str::to_owned));
    args.extend(protected_rules);
  }
  if !commands.is_empty() || !mcp.is_empty() {
    args.push("--allowedTools".to_owned());
    args.extend(commands.iter().map(|entry| format!("{COMMAND_TOOL}({entry}:*)")));
    args.extend(mcp.iter().map(|server| format!("mcp__{server}")));
  }
  args
}

/// `Edit(//<folder>/**)`, which keeps Claude Code's edit tools out of `folder`, an absolute path; `None`
/// for a path that is no text, which the invocation refuses before it gets here.
fn edit_rule(folder: &Path) -> Option<String> {
  let text = folder.to_str()?.replace('\\', "/");
  let text = text.strip_prefix("//?/").unwrap_or(&text);
  let posix = match text.as_bytes() {
    [drive, b':', ..] if drive.is_ascii_alphabetic() => {
      format!("/{}{}", char::from(drive.to_ascii_lowercase()), &text[2..])
    }
    _ => text.to_owned(),
  };
  Some(format!("Edit(/{}/**)", posix.trim_end_matches('/')))
}
