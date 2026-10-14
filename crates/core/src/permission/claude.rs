//! Claude Code (`claude -p`) arguments that enforce permissions (`.tmp/cli-probe/REPORT.md`,
//! "Recommended argv"), for [`crate::connection::invocation::ClaudeInvocation`].
//!
//! - `--tools` offers only the built-in tools the permissions need: reading and searching always, `Bash` always (every
//!   command either runs by a rule or asks a person), the edit tools for *Edit files in the task worktree*, the web
//!   tools only with network access. Scheduling, remote triggers, worktree switching and sub-agents are never offered.
//! - Agents ask (the user's decision 2026-10-01, "List + ask"): every invocation gets Orchester's permission prompt
//!   tool (`crate::connection::claude_approval`), and what no rule allows goes to a person instead of being denied. So
//!   no step runs in `dontAsk`: writing steps without allowed commands run in `acceptEdits`, every other step in
//!   `default`. In `default` every check that would ask, an unlisted command, a listed one with a `$VAR` expansion, a
//!   compound command (`;`, `&&`, `|`) with an unlisted part or a path outside the working folders, hands the call to
//!   the prompt tool; `dontAsk` turns exactly these into the denial "Permission to use Bash has been denied because
//!   Claude Code is running in don't ask mode" (2.1.89, 2.1.284 and 2.1.286: the only place the mode denies is where
//!   the check returned `ask`). A read-only step asks too: an approved command runs outside any sandbox and can change
//!   files, which the approval says; the edit tools stay removed.
//! - Allowed commands become `--allowedTools "Bash(<prefix>:*)"` prefix rules. Claude Code also runs the commands it
//!   classifies as read-only (`ls`, `cat`, `git log`, …) without a rule, in every permission mode (both 2.1.89 and
//!   2.1.284: the last check before asking). They change no file; the user accepted them as Codex's known-safe
//!   read-only commands are accepted (`docs/plan.md`, C4), and the option's text says so.
//! - *Ask before running commands*, and allowed commands without any entry, offer `Bash` with the settings
//!   `{"permissions":{"ask":["Bash"]}}` (`--settings`, which `--setting-sources ""` does not exclude). A rule asking
//!   for the whole tool is checked before the tool's own check, so neither `acceptEdits` nor the read-only classifier
//!   runs a command without the prompt (both versions: the whole-tool ask rule returns `ask` before
//!   `checkPermissions`): every command asks.
//! - `acceptEdits` runs `mkdir`, `touch`, `rm`, `rmdir`, `mv`, `cp` and `sed` in any form without a rule (both
//!   versions: a mode check after the deny, ask and allow rules), so a writing step with allowed commands does not run
//!   in it ([`Mechanism::ClaudeEditRules`]). It allows its edits with an `Edit(//<folder>/**)` rule for the worktree
//!   and each extra writable folder: both versions accept an edit there by such a rule as `acceptEdits` does in a
//!   working directory, after the same deny rules and safety checks, and a command's own path check (2.1.89 runs it
//!   before the allow rules) accepts a write there by the same rule. Those seven commands then run only in their listed
//!   forms; another form asks like every other command.
//! - Network off cannot be enforced (commands keep network access, and Claude Code's own sandbox is not supported on
//!   Windows), so network stays on; when off was chosen the web tools are still removed.
//! - The tools of the MCP servers the invocation gives the agent are allowed with one `mcp__<server>` rule per server;
//!   without a rule, print mode denies them. The servers themselves run outside these permissions.
//! - *Ask before MCP actions*: the MCP servers' tools have no `mcp__<server>` rules, so each call asks, except the
//!   tools a server lists to run without asking ([`crate::connection::mcp::ExemptTools`]): each gets an
//!   `mcp__<server>__<tool>` allow rule. 2.1.286 matches such a rule to exactly that tool of that server, and checks
//!   the allow rules before it calls the permission prompt tool, so these calls never reach a person; the server's
//!   other tools still have no rule and ask.
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

use super::{
  CommandPermission, NetworkAccess, RepositoryAccess,
  capability::{EnforcedPermissions, Mechanism, PermissionSetting},
};
use crate::connection::mcp::{InvocationExemptions, McpServerName};

const READ_TOOLS: [&str; 4] = ["Read", "Glob", "Grep", "TodoWrite"];
const EDIT_TOOLS: [&str; 3] = ["Edit", "Write", "NotebookEdit"];
const WEB_TOOLS: [&str; 2] = ["WebFetch", "WebSearch"];
const COMMAND_TOOL: &str = "Bash";
/// Settings that make every command ask.
const ASK_EVERY_COMMAND: &str = r#"{"permissions":{"ask":["Bash"]}}"#;

/// The folders a Claude Code invocation's permissions name.
#[derive(Clone, Copy, Debug)]
pub struct PermissionFolders<'a> {
  /// Where a writing step may edit: the worktree and the extra writable folders it is granted.
  pub editable: &'a [&'a Path],
  /// Folders the agent may read but must not change.
  pub protected: &'a [&'a Path],
}

/// The permission arguments of a Claude Code invocation that runs with `enforced`, gives the agent the MCP
/// servers `mcp`, whose tools `exempt` run without asking, and may change files only as `folders` say.
pub fn permission_args(
  enforced: &EnforcedPermissions,
  mcp: &[McpServerName],
  exempt: &InvocationExemptions,
  folders: PermissionFolders<'_>,
) -> Vec<String> {
  let permissions = enforced.permissions();
  let commands = match permissions.commands() {
    CommandPermission::Allowed(commands) => commands.entries(),
    // A person decides each command: the ask rule below sends every one to them.
    CommandPermission::AskBefore => &[],
    // Refused by `enforced_on` for Claude Code; nothing runs without a rule.
    CommandPermission::AnyInWorktree => &[],
  };
  let writes = permissions.repository() == RepositoryAccess::Write;
  // Without a listed command, every command asks a person.
  let ask_commands = commands.is_empty();
  let ask_tools = permissions.interactive_tool_approvals();
  let network = permissions.network() == NetworkAccess::On && enforced.network_stays_on().is_none();
  let edit_rules = enforced.mechanism(PermissionSetting::Repository) == Mechanism::ClaudeEditRules;
  let accepts_edits = writes && !edit_rules;
  let mut offered: Vec<&str> = READ_TOOLS.to_vec();
  let mut denied: Vec<String> = Vec::new();
  if writes {
    offered.extend(EDIT_TOOLS);
  } else {
    denied.extend(EDIT_TOOLS.map(str::to_owned));
  }
  // Commands always reach a person in the end, which `dontAsk` would deny before asking.
  let mode = if accepts_edits { "acceptEdits" } else { "default" };
  offered.push(COMMAND_TOOL);
  let mcp: Vec<String> = if ask_tools {
    exempt.claude_rules()
  } else {
    mcp.iter().map(|server| format!("mcp__{server}")).collect()
  };
  if network {
    offered.extend(WEB_TOOLS);
  } else {
    denied.extend(WEB_TOOLS.map(str::to_owned));
  }
  if writes {
    // Every edit tool is denied already in a read-only step.
    denied.extend(folders.protected.iter().filter_map(|folder| edit_rule(folder)));
  }
  let mut args = vec![
    "--tools".to_owned(),
    offered.join(","),
    "--permission-mode".to_owned(),
    mode.to_owned(),
  ];
  if ask_commands {
    args.extend(["--settings".to_owned(), ASK_EVERY_COMMAND.to_owned()]);
  }
  if !denied.is_empty() {
    args.push("--disallowedTools".to_owned());
    args.extend(denied);
  }
  if !commands.is_empty() || !mcp.is_empty() {
    args.push("--allowedTools".to_owned());
    args.extend(commands.iter().map(|entry| format!("{COMMAND_TOOL}({entry}:*)")));
    if edit_rules {
      args.extend(folders.editable.iter().filter_map(|folder| edit_rule(folder)));
    }
    args.extend(mcp);
  }
  args
}

/// Whether an `Edit` allow rule can let a step that edits by rule ([`Mechanism::ClaudeEditRules`])
/// edit in `folder`. Not on a network share, whose form in Claude Code's patterns is not documented:
/// every edit there would be denied, so the invocation refuses such a folder and the profile shows it.
pub fn edits_allowed_by_rule(folder: &Path) -> bool {
  edit_rule(folder).is_some()
}

/// `Edit(//<folder>/**)`: as a deny rule it keeps Claude Code's edit tools out of `folder`, as an
/// allow rule it lets them edit there. `folder` is an absolute path, written in the POSIX form
/// Claude Code matches permission paths in (`C:\repo` is `//c/repo`). `None` for a path that is no
/// text, which the invocation refuses before it gets here, and for a network path
/// (`\\server\share`), whose form in Claude Code's patterns is not documented: such a folder is kept
/// from edits only by its files being read-only, and where edits need an allow rule it gets none.
fn edit_rule(folder: &Path) -> Option<String> {
  path_rule("Edit", folder).map(|rule| format!("{rule}/**)"))
}

/// `Read(//<folder>/**)`: as a deny rule it keeps Claude Code's reading tools (`Read`, and as far as the CLI
/// applies Read rules to them, `Glob` and `Grep`) out of `folder`. `None` where [`edit_rule`] has none.
pub fn read_rule(folder: &Path) -> Option<String> {
  path_rule("Read", folder).map(|rule| format!("{rule}/**)"))
}

/// `Read(//<file>)`: as a deny rule it keeps Claude Code's reading tools from one file. `None` where [`edit_rule`]
/// has none.
pub fn read_file_rule(file: &Path) -> Option<String> {
  path_rule("Read", file).map(|rule| format!("{rule})"))
}

/// `<tool>(/<path>` without its end: `path` in the POSIX form Claude Code matches permission paths in.
fn path_rule(tool: &str, path: &Path) -> Option<String> {
  let text = path.to_str()?.replace('\\', "/");
  let text = text.strip_prefix("//?/").unwrap_or(&text);
  if text.starts_with("//") || text.starts_with("UNC/") {
    return None;
  }
  let posix = match text.as_bytes() {
    [drive, b':', ..] if drive.is_ascii_alphabetic() => {
      format!("/{}{}", char::from(drive.to_ascii_lowercase()), &text[2..])
    }
    _ => text.to_owned(),
  };
  Some(format!("{tool}(/{}", posix.trim_end_matches('/')))
}
