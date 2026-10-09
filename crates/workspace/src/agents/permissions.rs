//! How agent permissions read (`docs/design/orchester-workspace.md`, "Agent permissions"): the
//! options' keys, and the allowed commands as the editor adds and removes them. Which options are
//! enforceable is decided in core (`orchester_core::permission::capability`); this module only names
//! them. The sentences built from them are each app's.

use orchester_core::{
  connection::{AgentProvider, invocation::Platform},
  permission::{
    AllowedCommands, AllowedCommandsError, CommandPrefix, CommandPrefixError,
    capability::{Mechanism, PermissionOption, PermissionSetting, Support, support},
  },
};

use crate::messages::Message;

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

/// What an option allows on `provider` (`C85wh` in `lcwsy`). Claude Code and native agents also run
/// known-safe read-only commands besides an allowed list, so only their *Allowed commands* says so;
/// only Codex asks about commands that need more than its sandbox, so only its *Any command* says that.
pub fn option_detail(option: PermissionOption, provider: Option<AgentProvider>) -> &'static str {
  match option {
    PermissionOption::AllowedCommands if provider.is_some_and(AgentProvider::is_native) => {
      "agents.permissions.option.allowed.nativeDetail"
    }
    PermissionOption::AllowedCommands if runs_read_only_unlisted(provider) => {
      "agents.permissions.option.allowed.claudeDetail"
    }
    PermissionOption::AnyCommand if provider == Some(AgentProvider::CodexCli) => {
      "agents.permissions.option.any.codexDetail"
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

/// Whether `provider` enforces an allowed list the way that also runs known-safe read-only commands.
pub fn runs_read_only_unlisted(provider: Option<AgentProvider>) -> bool {
  provider.is_some_and(|provider| {
    support(provider, Platform::current(), PermissionOption::AllowedCommands)
      == Support::Enforced(Mechanism::ClaudeAllowedTools)
  })
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
