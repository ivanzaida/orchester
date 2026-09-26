//! How agent permissions read (`docs/design/orchester-workspace.md`, "Agent permissions"): the
//! options, their summary, why a connection cannot enforce an option and how it enforces the
//! ones it can. Which options are enforceable is decided in core
//! (`orchester_core::permission::capability`); this module only words it.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  connection::AgentProvider,
  permission::{
    AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess,
    capability::{EnforcedPermissions, Mechanism, PermissionOption, PermissionProblem, PermissionSetting, Unavailable},
  },
};

use crate::screens::project::connections::status::program_key;

/// Title of an option (`D2t8u` in `lcwsy`).
pub const fn option_title(option: PermissionOption) -> &'static str {
  match option {
    PermissionOption::ReadOnly => "agents.permissions.option.readOnly.title",
    PermissionOption::EditWorktree => "agents.permissions.option.editWorktree.title",
    PermissionOption::AskBeforeCommands => "agents.permissions.option.askBefore.title",
    PermissionOption::AllowedCommands => "agents.permissions.option.allowed.title",
    PermissionOption::AnyCommand => "agents.permissions.option.any.title",
    PermissionOption::NetworkOff => "agents.permissions.networkOff",
    PermissionOption::NetworkOn => "agents.permissions.networkOn",
  }
}

/// What an option allows (`C85wh` in `lcwsy`).
pub const fn option_detail(option: PermissionOption) -> &'static str {
  match option {
    PermissionOption::ReadOnly => "agents.permissions.option.readOnly.detail",
    PermissionOption::EditWorktree => "agents.permissions.option.editWorktree.detail",
    PermissionOption::AskBeforeCommands => "agents.permissions.option.askBefore.detail",
    PermissionOption::AllowedCommands => "agents.permissions.option.allowed.detail",
    PermissionOption::AnyCommand => "agents.permissions.option.any.detail",
    PermissionOption::NetworkOff => "agents.permissions.networkOffDetail",
    PermissionOption::NetworkOn => "agents.permissions.networkOnDetail",
  }
}

pub const fn setting_label(setting: PermissionSetting) -> &'static str {
  match setting {
    PermissionSetting::Repository => "agents.permissions.repository",
    PermissionSetting::Commands => "agents.permissions.commands",
    PermissionSetting::Network => "agents.permissions.network",
  }
}

/// Why `provider` cannot enforce an option (`NtPye`, `cOfD0` in 04.B), or `None` when the option
/// itself says it (asking before commands is marked *later*).
pub fn unavailable(ctx: &Ctx, reason: Unavailable, provider: AgentProvider) -> Option<Arc<str>> {
  let key = match reason {
    Unavailable::NeedsPermissionRequests => return None,
    Unavailable::CannotLimitCommands => "agents.permissions.unavailable.cannotLimitCommands",
    Unavailable::CannotConfineCommands => "agents.permissions.unavailable.cannotConfineCommands",
    Unavailable::CannotBlockNetwork => "agents.permissions.unavailable.cannotBlockNetwork",
  };
  Some(ctx.t_args(key, [("cli", &*ctx.t(program_key(provider)))]))
}

/// How a CLI enforces a setting, in its own terms (`s8kRYf`, `D1auE7`, `rRzit`).
pub fn mechanism(ctx: &Ctx, mechanism: Mechanism, permissions: &AgentPermissions) -> Arc<str> {
  let key = match mechanism {
    Mechanism::CodexReadOnlySandbox => "agents.permissions.mechanism.codexReadOnly",
    Mechanism::CodexWorkspaceWriteSandbox => "agents.permissions.mechanism.codexWorkspaceWrite",
    Mechanism::CodexSandboxedCommands => "agents.permissions.mechanism.codexCommands",
    Mechanism::CodexSandboxNetworkOff => "agents.permissions.mechanism.codexNetworkOff",
    Mechanism::CodexSandboxNetworkOn => "agents.permissions.mechanism.codexNetworkOn",
    Mechanism::ClaudeEditToolsRemoved => "agents.permissions.mechanism.claudeEditToolsRemoved",
    Mechanism::ClaudeAcceptEdits => "agents.permissions.mechanism.claudeAcceptEdits",
    Mechanism::ClaudeWebTools => "agents.permissions.mechanism.claudeWebTools",
    Mechanism::ClaudeAllowedTools => {
      let count = match permissions.commands() {
        CommandPermission::Allowed(commands) => commands.entries().len(),
        CommandPermission::AskBefore | CommandPermission::AnyInWorktree => 0,
      };
      let key = if count == 1 {
        "agents.permissions.mechanism.claudeAllowedTools.one"
      } else {
        "agents.permissions.mechanism.claudeAllowedTools.other"
      };
      return ctx.t_args(key, [("count", count.to_string().as_str())]);
    }
  };
  ctx.t(key)
}

/// Words of the three settings, long (`haXji`: *Edits files in the task worktree · 3 allowed
/// commands · Network off*) or compact for a run (`YOcfT`: *Edit files · 3 commands · Network
/// off*).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Length {
  Long,
  Compact,
}

/// The settings in one line. `network_stays_on`: off was chosen but the connection cannot enforce
/// it, so it reads *Network stays on*.
pub fn summary(ctx: &Ctx, permissions: &AgentPermissions, network_stays_on: bool, length: Length) -> Arc<str> {
  let (repository, commands, network) = parts(ctx, permissions, network_stays_on, length);
  ctx.t_args(
    "agents.permissions.summary",
    [
      ("repository", &*repository),
      ("commands", &*commands),
      ("network", &*network),
    ],
  )
}

/// The keys of one [`Length`].
struct Words {
  read_only: &'static str,
  edit: &'static str,
  commands_ask: &'static str,
  commands_any: &'static str,
  commands_none: &'static str,
  commands_one: &'static str,
  commands_other: &'static str,
  network_off: &'static str,
  network_stays_on: &'static str,
  network_on: &'static str,
}

const LONG: Words = Words {
  read_only: "agents.permissions.long.readOnly",
  edit: "agents.permissions.long.edit",
  commands_ask: "agents.permissions.long.commandsAsk",
  commands_any: "agents.permissions.long.commandsAny",
  commands_none: "agents.permissions.long.commandsNone",
  commands_one: "agents.permissions.long.commandsOne",
  commands_other: "agents.permissions.long.commandsOther",
  network_off: "agents.permissions.long.networkOff",
  network_stays_on: "agents.permissions.long.networkStaysOn",
  network_on: "agents.permissions.long.networkOn",
};

const COMPACT: Words = Words {
  read_only: "agents.permissions.compact.readOnly",
  edit: "agents.permissions.compact.edit",
  commands_ask: "agents.permissions.compact.commandsAsk",
  commands_any: "agents.permissions.compact.commandsAny",
  commands_none: "agents.permissions.compact.commandsNone",
  commands_one: "agents.permissions.compact.commandsOne",
  commands_other: "agents.permissions.compact.commandsOther",
  network_off: "agents.permissions.compact.networkOff",
  network_stays_on: "agents.permissions.compact.networkStaysOn",
  network_on: "agents.permissions.compact.networkOn",
};

/// The three settings' words, for a summary.
pub fn parts(
  ctx: &Ctx,
  permissions: &AgentPermissions,
  network_stays_on: bool,
  length: Length,
) -> (Arc<str>, Arc<str>, Arc<str>) {
  let words = match length {
    Length::Long => &LONG,
    Length::Compact => &COMPACT,
  };
  let repository = ctx.t(match permissions.repository() {
    RepositoryAccess::ReadOnly => words.read_only,
    RepositoryAccess::Write => words.edit,
  });
  let commands = match permissions.commands() {
    CommandPermission::AskBefore => ctx.t(words.commands_ask),
    CommandPermission::AnyInWorktree => ctx.t(words.commands_any),
    CommandPermission::Allowed(commands) => match commands.entries().len() {
      0 => ctx.t(words.commands_none),
      1 => ctx.t(words.commands_one),
      count => ctx.t_args(words.commands_other, [("count", count.to_string().as_str())]),
    },
  };
  let network = ctx.t(match (permissions.network(), network_stays_on) {
    (NetworkAccess::Off, false) => words.network_off,
    (NetworkAccess::Off, true) => words.network_stays_on,
    (NetworkAccess::On, _) => words.network_on,
  });
  (repository, commands, network)
}

/// The summary of permissions as `provider` runs them: network off it cannot enforce reads as
/// staying on. Permissions it cannot run at all read as chosen.
pub fn enforced_summary(
  ctx: &Ctx,
  permissions: &AgentPermissions,
  enforced: Option<&EnforcedPermissions>,
  length: Length,
) -> Arc<str> {
  let stays_on = enforced.is_some_and(|enforced| enforced.network_stays_on().is_some());
  summary(ctx, permissions, stays_on, length)
}

/// Why a profile cannot be published or run with these permissions on its connection.
pub fn problem(ctx: &Ctx, problem: &PermissionProblem, provider: AgentProvider) -> Arc<str> {
  let cli = ctx.t(program_key(provider));
  match problem {
    PermissionProblem::NoEnforceableOption { setting } => ctx.t_args(
      "agents.permissions.problem.noEnforceableOption",
      [("cli", &*cli), ("setting", &*ctx.t(setting_label(*setting)))],
    ),
    PermissionProblem::NotEnforceable { option, .. } => ctx.t_args(
      "agents.permissions.problem.notEnforceable",
      [("cli", &*cli), ("option", &*ctx.t(option_title(*option)))],
    ),
  }
}
