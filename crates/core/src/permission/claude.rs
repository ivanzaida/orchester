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
//!   probe: it can change files, so a read-only step must not have it). With some, Claude Code also runs the commands
//!   it classifies as read-only (`ls`, `cat`, `git log`, …) without a rule, in every permission mode (both 2.1.89 and
//!   2.1.284: the last check before asking). They change no file; the user accepted them as Codex's known-safe
//!   read-only commands are accepted (`docs/plan.md`, C4), and the option's text says so.
//! - `acceptEdits` also runs `mkdir`, `touch`, `rm`, `rmdir`, `mv`, `cp` and `sed` in any form without a rule (both
//!   versions: a mode check after the deny, ask and allow rules). A writing step therefore denies each of them with a
//!   `Bash(<command>:*)` rule. A deny rule matching a command's prefix wins over every allow rule (both versions check
//!   prefix deny rules first, also after stripping variable assignments, wrappers and `xargs`), so it would deny a
//!   listed `sed -n` as well: a writing step whose allowed commands contain one of these words does not run in
//!   `acceptEdits` at all ([`Mechanism::ClaudeEditRules`]). It runs in `dontAsk` (`default` when a person is asked) and
//!   allows its edits with an `Edit(//<folder>/**)` rule for the worktree and each extra writable folder: both versions
//!   accept an edit there by such a rule as `acceptEdits` does in a working directory, after the same deny rules and
//!   safety checks, and a command's own path check (2.1.89 runs it before the allow rules) accepts a write there by the
//!   same rule. Then only the listed prefixes of those commands run.
//! - Network off cannot be enforced (commands keep network access, and Claude Code's own sandbox is not supported on
//!   Windows), so network stays on; when off was chosen the web tools are still removed.
//! - The tools of the MCP servers the invocation gives the agent are allowed with one `mcp__<server>` rule per server;
//!   without a rule, print mode denies them. The servers themselves run outside these permissions.
//! - Asking a person (*Ask before running commands*, *Ask before MCP actions*): the invocation gets Orchester's
//!   permission prompt tool (`crate::connection::claude_approval`), and what the rules leave open goes to it instead of
//!   being denied. The MCP servers' tools then have no `mcp__<server>` rules, and a read-only step runs in `default`
//!   rather than `dontAsk`, which would deny before asking.
//! - Asking before commands in a writing step offers `Bash` with the settings `{"permissions":{"ask":["Bash"]}}`
//!   (`--settings`, which `--setting-sources ""` does not exclude). A rule asking for the whole tool is checked before
//!   the tool's own check, so neither `acceptEdits` nor the read-only classifier runs a command without the prompt
//!   (both versions: the whole-tool ask rule returns `ask` before `checkPermissions`). A read-only step never gets
//!   `Bash`: an approved command would run outside any sandbox and could write.
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
  CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
  capability::{EnforcedPermissions, Mechanism, PermissionSetting},
};
use crate::connection::mcp::McpServerName;

const READ_TOOLS: [&str; 4] = ["Read", "Glob", "Grep", "TodoWrite"];
const EDIT_TOOLS: [&str; 3] = ["Edit", "Write", "NotebookEdit"];
const WEB_TOOLS: [&str; 2] = ["WebFetch", "WebSearch"];
const COMMAND_TOOL: &str = "Bash";
/// Commands `acceptEdits` runs without a rule.
const MODE_ALLOWED: [&str; 7] = ["mkdir", "touch", "rm", "rmdir", "mv", "cp", "sed"];
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
/// servers `mcp` and may change files only as `folders` say.
pub fn permission_args(
  enforced: &EnforcedPermissions,
  mcp: &[McpServerName],
  folders: PermissionFolders<'_>,
) -> Vec<String> {
  let permissions = enforced.permissions();
  let commands = match permissions.commands() {
    CommandPermission::Allowed(commands) => commands.entries(),
    // A person decides each command; the ask rule below sends every one to them.
    CommandPermission::AskBefore => &[],
    // Refused by `enforced_on` for Claude Code; nothing runs without a rule.
    CommandPermission::AnyInWorktree => &[],
  };
  let writes = permissions.repository() == RepositoryAccess::Write;
  let ask_commands = writes && *permissions.commands() == CommandPermission::AskBefore;
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
  let mode = match (accepts_edits, ask_tools) {
    (true, _) => "acceptEdits",
    (false, true) => "default",
    (false, false) => "dontAsk",
  };
  if commands.is_empty() && !ask_commands {
    denied.push(COMMAND_TOOL.to_owned());
  } else {
    offered.push(COMMAND_TOOL);
  }
  if accepts_edits && !commands.is_empty() {
    denied.extend(MODE_ALLOWED.map(|command| format!("{COMMAND_TOOL}({command}:*)")));
  }
  let mcp = if ask_tools { &[] } else { mcp };
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
    args.extend(mcp.iter().map(|server| format!("mcp__{server}")));
  }
  args
}

/// Whether an allowed command contains a word `acceptEdits` runs without a rule. A
/// `Bash(<command>:*)` deny rule could then match a listed command, so a writing step with
/// `commands` does not run in `acceptEdits` ([`Mechanism::ClaudeEditRules`]).
pub(super) fn lists_file_commands(commands: &[CommandPrefix]) -> bool {
  commands
    .iter()
    .any(|entry| entry.as_str().split(' ').any(|word| MODE_ALLOWED.contains(&word)))
}

/// `Edit(//<folder>/**)`: as a deny rule it keeps Claude Code's edit tools out of `folder`, as an
/// allow rule it lets them edit there. `folder` is an absolute path, written in the POSIX form
/// Claude Code matches permission paths in (`C:\repo` is `//c/repo`). `None` for a path that is no
/// text, which the invocation refuses before it gets here, and for a network path
/// (`\\server\share`), whose form in Claude Code's patterns is not documented: such a folder is kept
/// from edits only by its files being read-only, and where edits need an allow rule it gets none.
fn edit_rule(folder: &Path) -> Option<String> {
  let text = folder.to_str()?.replace('\\', "/");
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
  Some(format!("Edit(/{}/**)", posix.trim_end_matches('/')))
}
