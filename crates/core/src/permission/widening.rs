//! What widens what agents may do, beyond what they were given: broader profile permissions, and project
//! environment variables that change which programs agents and command tools load, where they connect or
//! where their tools keep their configuration. Automation may narrow freely; widening is a person's
//! decision unless a person lets automation make it (*Let automation change agent permissions*).

use super::{AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess};

/// One way permissions are broader than those they are compared with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Widening {
  /// Write access to the repository where it was read-only.
  Repository,
  /// A writable folder outside the worktree that was not granted.
  WritableFolder,
  /// Commands that run where they did not: any command, a prefix not allowed, or an allowed list or
  /// asking about commands in place of the other.
  Commands,
  /// Network access where it was off.
  Network,
  /// Tool calls no longer asked about.
  ToolCallsUnasked,
  /// Approving automatically where a person was asked.
  AutoApprove,
}

impl AgentPermissions {
  /// How these permissions are broader than `base`; empty when they are the same or narrower.
  pub fn widenings_from(&self, base: &Self) -> Vec<Widening> {
    let granted = |permissions: &Self| {
      permissions
        .granted_folders()
        .map(|folders| folders.entries().to_vec())
        .unwrap_or_default()
    };
    let base_folders = granted(base);
    [
      (
        self.repository == RepositoryAccess::Write && base.repository == RepositoryAccess::ReadOnly,
        Widening::Repository,
      ),
      (
        granted(self).iter().any(|folder| !base_folders.contains(folder)),
        Widening::WritableFolder,
      ),
      (commands_widen(&base.commands, &self.commands), Widening::Commands),
      (
        self.network == NetworkAccess::On && base.network == NetworkAccess::Off,
        Widening::Network,
      ),
      (
        base.interactive_tool_approvals && !self.interactive_tool_approvals,
        Widening::ToolCallsUnasked,
      ),
      (self.auto_approve && !base.auto_approve, Widening::AutoApprove),
    ]
    .into_iter()
    .filter_map(|(widens, widening)| widens.then_some(widening))
    .collect()
  }
}

/// Whether `next` lets commands run that `base` does not. Any command in the worktree is the widest; an
/// allowed list and asking a person are not comparable, so trading one for the other widens, except for
/// an empty list, which runs nothing a person was not asked about.
fn commands_widen(base: &CommandPermission, next: &CommandPermission) -> bool {
  match (base, next) {
    (CommandPermission::AnyInWorktree, _) => false,
    (_, CommandPermission::AnyInWorktree) => true,
    (CommandPermission::Allowed(base), CommandPermission::Allowed(next)) => {
      next.entries().iter().any(|command| !base.entries().contains(command))
    }
    (CommandPermission::AskBefore, CommandPermission::Allowed(next)) => !next.entries().is_empty(),
    (CommandPermission::Allowed(_), CommandPermission::AskBefore) => true,
    (CommandPermission::AskBefore, CommandPermission::AskBefore) => false,
  }
}

/// Variables that change which programs load and how (search paths, preloads, interpreter options, shell
/// start files), where agents and tools connect (model endpoints, proxies), or where they keep their
/// configuration (config and tool homes). Names are compared ignoring case, as Windows does.
const LOADING_NAMES: &[&str] = &[
  "ALL_PROXY",
  "APPDATA",
  "BASH_ENV",
  "CDPATH",
  "CLAUDE_CONFIG_DIR",
  "COMSPEC",
  "ENV",
  "GIT_ASKPASS",
  "GIT_EXEC_PATH",
  "GIT_SSH",
  "GIT_SSH_COMMAND",
  "HOME",
  "JAVA_TOOL_OPTIONS",
  "LOCALAPPDATA",
  "NODE_OPTIONS",
  "NODE_PATH",
  "NO_PROXY",
  "PATH",
  "PATHEXT",
  "PERL5LIB",
  "PERL5OPT",
  "PROMPT_COMMAND",
  "PSMODULEPATH",
  "PYTHONHOME",
  "PYTHONPATH",
  "PYTHONSTARTUP",
  "RUBYLIB",
  "RUBYOPT",
  "RUSTC_WRAPPER",
  "SHELL",
  "SSH_ASKPASS",
  "USERPROFILE",
  "ZDOTDIR",
  "_JAVA_OPTIONS",
];

/// Prefixes of variable families that do the same: the dynamic loaders' and git's configuration.
const LOADING_PREFIXES: &[&str] = &["DYLD_", "GIT_CONFIG", "LD_", "NPM_CONFIG_"];

/// Suffixes of the same: model endpoints, proxies and tool homes (`OPENAI_BASE_URL`, `HTTPS_PROXY`,
/// `CODEX_HOME`).
const LOADING_SUFFIXES: &[&str] = &["_BASE_URL", "_HOME", "_PROXY"];

/// Whether the environment variable `name` changes which programs agents and command tools load, where
/// they connect or where their tools keep their configuration, so setting it widens what they may do.
pub fn changes_loading(name: &str) -> bool {
  let name = name.trim().to_ascii_uppercase();
  LOADING_NAMES.contains(&name.as_str())
    || LOADING_PREFIXES.iter().any(|prefix| name.starts_with(prefix))
    || LOADING_SUFFIXES.iter().any(|suffix| name.ends_with(suffix))
}
