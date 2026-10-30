//! What widens what agents may do, beyond what they were given: broader profile permissions, a command tool's
//! command that runs what it did not, and changes to project environment variables, which can change which
//! programs agents and command tools load, where they connect or where their tools keep their configuration
//! ([`harmless_variable`] names the few that cannot). Automation may narrow freely; widening is a person's
//! decision unless a person lets automation make it (*Let automation change agent permissions*).

use std::collections::{BTreeMap, BTreeSet};

use super::{AgentPermissions, CommandPermission, RepositoryAccess};
use crate::resource::{CommandAction, CommandEnvironment};

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
  /// Network access to more than before: on where it was off or limited to hosts, or a host the previous list did
  /// not cover ([`super::NetworkAccess::covers`]).
  Network,
  /// Tool calls no longer asked about.
  ToolCallsUnasked,
  /// Approving automatically where a person was asked.
  AutoApprove,
  /// Autopilot where the approval mode was Manual ([`super::ApprovalMode::is_stricter_than`]).
  Autopilot,
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
      (!base.network.covers(&self.network), Widening::Network),
      (
        base.interactive_tool_approvals && !self.interactive_tool_approvals,
        Widening::ToolCallsUnasked,
      ),
      (self.auto_approve && !base.auto_approve, Widening::AutoApprove),
      (
        base.approval_mode.is_stricter_than(self.approval_mode),
        Widening::Autopilot,
      ),
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

/// One way a command runs what the command it is compared with did not.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CommandWidening {
  /// Another program.
  Program,
  /// Other arguments.
  Arguments,
  /// Exit codes that pass now and did not, in order.
  ExitCodes(Vec<i64>),
  /// A variable set, changed or removed that is not harmless ([`changed_variables`]), by name.
  Variable(String),
}

impl CommandAction {
  /// What this command runs that `before` did not: another program, other arguments, exit codes that now pass,
  /// or variables changed but harmless ones. Narrower exit codes, the timeout and the output limit change nothing
  /// a person must decide.
  pub fn widenings_from(&self, before: &Self) -> Vec<CommandWidening> {
    let mut widenings = Vec::new();
    if before.program() != self.program() {
      widenings.push(CommandWidening::Program);
    }
    if before.args() != self.args() {
      widenings.push(CommandWidening::Arguments);
    }
    let passing: Vec<i64> = self
      .exit_codes()
      .iter()
      .filter(|code| !before.exit_codes().contains(*code))
      .collect();
    if !passing.is_empty() {
      widenings.push(CommandWidening::ExitCodes(passing));
    }
    widenings.extend(
      changed_variables(before.environment(), self.environment())
        .into_iter()
        .map(CommandWidening::Variable),
    );
    widenings
  }
}

/// The names of the variables `new` changes from `old` but harmless ones, in name order: a removal included,
/// since removing a person's proxy or certificate setting can widen as well.
pub fn changed_variables(old: &CommandEnvironment, new: &CommandEnvironment) -> Vec<String> {
  let old: BTreeMap<String, &str> = old.iter().map(|(name, value)| (name.to_string(), value)).collect();
  let new: BTreeMap<String, &str> = new.iter().map(|(name, value)| (name.to_string(), value)).collect();
  old
    .keys()
    .chain(new.keys())
    .collect::<BTreeSet<_>>()
    .into_iter()
    .filter(|name| old.get(*name) != new.get(*name) && !harmless_variable(name))
    .cloned()
    .collect()
}

/// Variables that only change how programs log, color their output, or tell the time and the language:
/// the ones automation may set or remove without a person. Names are compared ignoring case, as Windows
/// does.
const HARMLESS_NAMES: &[&str] = &[
  "CI",
  "FORCE_COLOR",
  "LANG",
  "LC_ALL",
  "LC_COLLATE",
  "LC_CTYPE",
  "LC_MESSAGES",
  "LC_MONETARY",
  "LC_NUMERIC",
  "LC_TIME",
  "NO_COLOR",
  "RUST_BACKTRACE",
  "RUST_LOG",
  "TZ",
];

/// Whether setting or removing the environment variable `name` cannot change what agents and command
/// tools may do. Any other variable may: it can change which programs load, where they connect, what they
/// trust or where their tools keep their configuration, and removing one a person set (a proxy, say) can
/// widen as well, so it is a person's decision. The list is short on purpose: what it leaves out is refused.
pub fn harmless_variable(name: &str) -> bool {
  let name = name.trim().to_ascii_uppercase();
  HARMLESS_NAMES.contains(&name.as_str())
}
