//! Always allow (`NV6xJ`, `XKja4`): a command's prefix goes into its profile's Allowed commands, in a
//! new profile version; an MCP tool goes into its server's Tools that run without asking, a connection
//! setting. Both start at once: Orchester answers the rest of the run (a command) or of the invocation
//! (an MCP tool) itself until the agent's CLI gets the rule when an agent starts.

use thiserror::Error;

use crate::{
  connection::mcp::{ExemptTools, ExemptToolsError, McpToolName},
  permission::{
    AgentPermissions, AllowedCommands, AllowedCommandsError, CommandPermission, CommandPrefix,
    category::{ClassifiedCommand, CommandCategory},
  },
};

/// Why Always allow is unavailable for a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlwaysUnavailable {
  /// Part of it is not read to the end.
  Unknown,
  /// Several parts would ask by themselves: no one prefix covers it.
  SeveralAsk,
  /// It rewrites or discards history; Allow is unavailable for that category.
  Destructive,
  /// It reads a credential location or store.
  Secrets,
  /// Every prefix it offers would also allow a more dangerous form: a History / destructive one (`git
  /// reset` also allows `git reset --hard`), one that starts another program or writes outside (`git
  /// fetch` also allows `git fetch --upload-pack=…`), or one that reads a credential (`cat`).
  WiderForms,
  /// The profile asks before every command, or its CLI cannot keep a list (Codex): it has no Allowed
  /// commands to add to.
  NoAllowedList,
  /// The profile lists as many commands as a profile can.
  ListFull,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AlwaysAllowError {
  #[error("Always allow does not offer this prefix for the command")]
  NotOffered,
  #[error("the profile has no Allowed commands to add to")]
  NoAllowedList,
  #[error("{0}")]
  Commands(#[from] AllowedCommandsError),
  #[error("{0}")]
  Tools(#[from] ExemptToolsError),
}

/// The prefixes Always allow offers for `command` in a profile whose commands are `commands`, widest
/// first: the program and its subcommand, then the whole command.
pub fn always_prefixes(
  command: &ClassifiedCommand,
  commands: &CommandPermission,
) -> Result<Vec<CommandPrefix>, AlwaysUnavailable> {
  let CommandPermission::Allowed(listed) = commands else {
    return Err(AlwaysUnavailable::NoAllowedList);
  };
  if command
    .parts()
    .iter()
    .any(|part| part.category() == CommandCategory::Unknown)
  {
    return Err(AlwaysUnavailable::Unknown);
  }
  if command.category() == CommandCategory::HistoryDestructive {
    return Err(AlwaysUnavailable::Destructive);
  }
  if command.reads_secrets() {
    return Err(AlwaysUnavailable::Secrets);
  }
  if command.asking().count() > 1 {
    return Err(AlwaysUnavailable::SeveralAsk);
  }
  let part = command.forcing().ok_or(AlwaysUnavailable::Unknown)?;
  if part.has_wider_forms() {
    return Err(AlwaysUnavailable::WiderForms);
  }
  if listed.entries().len() >= AllowedCommands::MAX_ENTRIES {
    return Err(AlwaysUnavailable::ListFull);
  }
  let prefixes = part.prefixes();
  if prefixes.is_empty() {
    return Err(AlwaysUnavailable::Unknown);
  }
  Ok(prefixes)
}

/// `permissions` with `prefix` added to their Allowed commands, after checking that Always allow
/// offers it for `command`. A prefix already listed leaves them as they are.
pub fn allowing_command(
  permissions: &AgentPermissions,
  command: &ClassifiedCommand,
  prefix: &CommandPrefix,
) -> Result<AgentPermissions, AlwaysAllowError> {
  let offered = always_prefixes(command, permissions.commands()).map_err(|unavailable| match unavailable {
    AlwaysUnavailable::NoAllowedList => AlwaysAllowError::NoAllowedList,
    _ => AlwaysAllowError::NotOffered,
  })?;
  if !offered.contains(prefix) {
    return Err(AlwaysAllowError::NotOffered);
  }
  listing(permissions, prefix)
}

/// `permissions` with `prefix` listed too; a profile without a list cannot get one this way. A draft of
/// the profile gets the prefix this way, whatever else it changed.
pub fn listing(permissions: &AgentPermissions, prefix: &CommandPrefix) -> Result<AgentPermissions, AlwaysAllowError> {
  let CommandPermission::Allowed(listed) = permissions.commands() else {
    return Err(AlwaysAllowError::NoAllowedList);
  };
  if listed.entries().contains(prefix) {
    return Ok(permissions.clone());
  }
  let mut entries = listed.entries().to_vec();
  entries.push(prefix.clone());
  let mut allowing = permissions.clone();
  allowing.set_commands(CommandPermission::Allowed(AllowedCommands::new(entries)?));
  Ok(allowing)
}

/// `tools` with `tool` listed too; a tool already listed leaves them as they are.
pub fn allowing_tool(tools: &ExemptTools, tool: &str) -> Result<ExemptTools, AlwaysAllowError> {
  if tools.contains(tool) {
    return Ok(tools.clone());
  }
  let name = McpToolName::new(tool).map_err(ExemptToolsError::from)?;
  let mut names = tools.tools().to_vec();
  names.push(name);
  Ok(ExemptTools::new(names)?)
}
