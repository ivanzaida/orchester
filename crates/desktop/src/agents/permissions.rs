//! How agent permissions read (`docs/design/orchester-workspace.md`, "Agent permissions"): the
//! options, their summary, why a connection cannot enforce an option and how it enforces the
//! ones it can. Which options are enforceable is decided in core
//! (`orchester_core::permission::capability`); this module only words it.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  connection::{AgentProvider, invocation::Platform},
  permission::{
    AgentPermissions, AllowedCommands, AllowedCommandsError, CommandPermission, CommandPrefix, CommandPrefixError,
    NetworkAccess, RepositoryAccess,
    capability::{
      EnforcedPermissions, Mechanism, PermissionOption, PermissionProblem, PermissionSetting, Support, Unavailable,
      support,
    },
  },
};

use crate::{messages::Message, screens::project::connections::status::program_key};

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

/// What an option allows on `provider` (`C85wh` in `lcwsy`). Only Claude Code also runs known-safe
/// read-only commands besides an allowed list, so only its *Allowed commands* says so.
pub fn option_detail(option: PermissionOption, provider: Option<AgentProvider>) -> &'static str {
  match option {
    PermissionOption::AllowedCommands if runs_read_only_unlisted(provider) => {
      "agents.permissions.option.allowed.claudeDetail"
    }
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
    Mechanism::CodexCommandApprovals => "agents.permissions.mechanism.codexCommandApprovals",
    Mechanism::CodexSandboxNetworkOff => "agents.permissions.mechanism.codexNetworkOff",
    Mechanism::CodexSandboxNetworkOn => "agents.permissions.mechanism.codexNetworkOn",
    // Without a listed command every command asks.
    Mechanism::ClaudeEditToolsRemoved if lists_no_command(permissions) => {
      "agents.permissions.mechanism.claudeEditToolsRemovedAsk"
    }
    Mechanism::ClaudeEditToolsRemoved => "agents.permissions.mechanism.claudeEditToolsRemoved",
    Mechanism::ClaudeAcceptEdits => "agents.permissions.mechanism.claudeAcceptEdits",
    Mechanism::ClaudeEditRules => "agents.permissions.mechanism.claudeEditRules",
    Mechanism::ClaudeWebTools => "agents.permissions.mechanism.claudeWebTools",
    Mechanism::ClaudeWebToolsRemoved => "agents.permissions.mechanism.claudeWebToolsRemoved",
    Mechanism::CodexWritableRoots => "agents.permissions.mechanism.codexWritableRoots",
    Mechanism::ClaudeAddDir => "agents.permissions.mechanism.claudeAddDir",
    Mechanism::ClaudePermissionPrompt => "agents.permissions.mechanism.claudePermissionPrompt",
    Mechanism::ClaudeAllowedTools => {
      let count = match permissions.commands() {
        CommandPermission::Allowed(commands) => commands.entries().len(),
        CommandPermission::AskBefore | CommandPermission::AnyInWorktree => 0,
      };
      let key = if count == 0 {
        "agents.permissions.mechanism.claudeAllowedTools.none"
      } else if count == 1 {
        "agents.permissions.mechanism.claudeAllowedTools.one"
      } else {
        "agents.permissions.mechanism.claudeAllowedTools.other"
      };
      return ctx.t_args(key, [("count", count.to_string().as_str())]);
    }
  };
  ctx.t(key)
}

/// What a mechanism does beyond its setting's words, as a line under it; `None` for most.
pub fn mechanism_detail(ctx: &Ctx, mechanism: Mechanism, permissions: &AgentPermissions) -> Option<Arc<str>> {
  match (mechanism, permissions.commands()) {
    // Codex asks a person about a command that needs more than its sandbox (App Server).
    (Mechanism::CodexSandboxedCommands | Mechanism::CodexCommandApprovals, _) => Some(ctx.t(ESCALATIONS)),
    (Mechanism::ClaudeAllowedTools, CommandPermission::Allowed(list)) if !list.is_empty() => {
      Some(ctx.t(READ_ONLY_COMMANDS))
    }
    _ => None,
  }
}

/// That Claude Code also runs known-safe read-only commands without a rule (`docs/plan.md`, C4), and
/// asks a person about any other command.
const READ_ONLY_COMMANDS: &str = "agents.permissions.claudeReadOnlyCommands";

/// What runs besides `list` on `provider`: Claude Code's known-safe read-only commands, once the
/// list has a command (without one it gets no command tool), and that the others ask a person.
/// `None` where only the list runs.
pub fn unlisted_commands(ctx: &Ctx, provider: Option<AgentProvider>, list: &AllowedCommands) -> Option<Arc<str>> {
  (runs_read_only_unlisted(provider) && !list.is_empty()).then(|| ctx.t(READ_ONLY_COMMANDS))
}

/// That Codex asks a person about a command that needs more than its sandbox.
const ESCALATIONS: &str = "agents.permissions.codexEscalations";

/// That a command needing more than the sandbox asks a person, where `provider` enforces `commands`
/// with its sandbox; `None` elsewhere.
pub fn escalations(ctx: &Ctx, provider: Option<AgentProvider>, commands: &CommandPermission) -> Option<Arc<str>> {
  let option = PermissionOption::of_commands(commands);
  let sandboxed = provider.is_some_and(|provider| {
    matches!(
      support(provider, Platform::current(), option),
      Support::Enforced(Mechanism::CodexSandboxedCommands | Mechanism::CodexCommandApprovals)
    )
  });
  sandboxed.then(|| ctx.t(ESCALATIONS))
}

/// Whether `provider` enforces an allowed list the way that also runs known-safe read-only commands.
fn runs_read_only_unlisted(provider: Option<AgentProvider>) -> bool {
  provider.is_some_and(|provider| {
    support(provider, Platform::current(), PermissionOption::AllowedCommands)
      == Support::Enforced(Mechanism::ClaudeAllowedTools)
  })
}

/// For each entry of `list` whose first word changes files (`rm`, `sed`, …), that it also allows
/// any arguments after it, including ones that change other files.
pub fn file_command_hints(ctx: &Ctx, list: &AllowedCommands) -> Vec<Arc<str>> {
  list
    .entries()
    .iter()
    .filter(|entry| entry.starts_with_file_command())
    .map(|entry| ctx.t_args("agents.permissions.fileCommandPrefix", [("command", entry.as_str())]))
    .collect()
}

/// Whether no command is listed: asking before commands, or an empty list.
fn lists_no_command(permissions: &AgentPermissions) -> bool {
  match permissions.commands() {
    CommandPermission::Allowed(commands) => commands.is_empty(),
    CommandPermission::AskBefore => true,
    CommandPermission::AnyInWorktree => false,
  }
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
  let summary = ctx.t_args(
    "agents.permissions.summary",
    [
      ("repository", &*repository),
      ("commands", &*commands),
      ("network", &*network),
    ],
  );
  if permissions.interactive_tool_approvals() {
    ctx.t_args("agents.permissions.summaryWithTools", [("permissions", &*summary)])
  } else {
    summary
  }
}

/// The keys of one [`Length`].
struct Words {
  read_only: &'static str,
  edit: &'static str,
  edit_folders_one: &'static str,
  edit_folders_other: &'static str,
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
  edit_folders_one: "agents.permissions.long.editFoldersOne",
  edit_folders_other: "agents.permissions.long.editFoldersOther",
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
  edit_folders_one: "agents.permissions.compact.editFoldersOne",
  edit_folders_other: "agents.permissions.compact.editFoldersOther",
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
  // Extra writable folders widen what an agent that edits files may write, so the summary says so.
  let folders = permissions
    .granted_folders()
    .map_or(0, |folders| folders.entries().len());
  let repository = match (permissions.repository(), folders) {
    (RepositoryAccess::ReadOnly, _) => ctx.t(words.read_only),
    (RepositoryAccess::Write, 0) => ctx.t(words.edit),
    (RepositoryAccess::Write, 1) => ctx.t(words.edit_folders_one),
    (RepositoryAccess::Write, count) => ctx.t_args(words.edit_folders_other, [("count", count.to_string().as_str())]),
  };
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

/// `list` with the command `typed` added, or why it cannot be.
pub fn with_command(list: &AllowedCommands, typed: &str) -> Result<AllowedCommands, Message> {
  let prefix = CommandPrefix::new(typed).map_err(|error| match error {
    CommandPrefixError::Blank => Message::new("agents.permissions.commandInvalid.blank"),
    CommandPrefixError::TooLong => {
      Message::new("agents.permissions.commandInvalid.tooLong").with("max", CommandPrefix::MAX_CHARS.to_string())
    }
    CommandPrefixError::ForbiddenCharacter(character) => {
      Message::new("agents.permissions.commandInvalid.character").with("character", character.to_string())
    }
  })?;
  let mut entries = list.entries().to_vec();
  entries.push(prefix);
  AllowedCommands::new(entries).map_err(|error| match error {
    AllowedCommandsError::Duplicate(command) => {
      Message::new("agents.permissions.commandInvalid.duplicate").with("command", command.to_string())
    }
    AllowedCommandsError::TooMany => {
      Message::new("agents.permissions.commandInvalid.tooMany").with("max", AllowedCommands::MAX_ENTRIES.to_string())
    }
  })
}

/// `list` without the command `removed`.
pub fn without_command(list: &AllowedCommands, removed: &CommandPrefix) -> AllowedCommands {
  let rest = list.entries().iter().filter(|kept| *kept != removed).cloned().collect();
  AllowedCommands::new(rest).expect("a shorter list keeps its entries unique")
}
