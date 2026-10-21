//! Permission narrowing choices and validation, shared by the inspector and setup tools.
use orchester_core::{
  connection::invocation::Platform,
  permission::{
    AllowedCommands, CommandPermission, NetworkAccess, StepCommands, StepNetwork, StepPermissions,
    capability::{PermissionOption, support},
  },
};

use super::data::ProfileChoice;
use crate::{agents::permissions::with_command, messages::Message};

/// Whether this profile permits the step to write.
pub fn can_write(choice: &ProfileChoice) -> bool {
  choice.permissions.repository() == orchester_core::resource::RepositoryAccess::Write
}
/// Whether this connection enforces turning network access off.
pub fn can_block_network(choice: &ProfileChoice) -> bool {
  choice.permissions.network() == NetworkAccess::On && enforced(choice, PermissionOption::NetworkOff)
}
/// The profile's commands that this connection lets a step narrow.
pub fn commands(choice: &ProfileChoice) -> Vec<String> {
  match choice.permissions.commands() {
    CommandPermission::Allowed(list) if enforced(choice, PermissionOption::AllowedCommands) => {
      list.entries().iter().map(ToString::to_string).collect()
    }
    _ => Vec::new(),
  }
}
fn enforced(choice: &ProfileChoice, option: PermissionOption) -> bool {
  choice
    .provider
    .is_some_and(|p| support(p, Platform::current(), option).is_enforced())
}

/// Restores command and network inheritance without dropping the step's tool-approval gate or its choice about base
/// moves. Saving and publishing still use the ordinary widening checks.
pub fn inherit_profile(current: &StepPermissions) -> StepPermissions {
  StepPermissions {
    interactive_tool_approvals: current.interactive_tool_approvals,
    takes_base_moves: current.takes_base_moves,
    ..StepPermissions::default()
  }
}

/// Validates only the narrowing the profile and its connection support on this platform.
pub fn permissions(
  only_commands: Option<&[String]>,
  network_off: bool,
  ask_before_tool_calls: bool,
  choice: &ProfileChoice,
) -> Result<StepPermissions, Message> {
  let enforced = |option| {
    choice
      .provider
      .is_some_and(|provider| support(provider, Platform::current(), option).is_enforced())
  };
  let mut permissions = StepPermissions {
    interactive_tool_approvals: ask_before_tool_calls,
    ..StepPermissions::default()
  };
  if let Some(commands) = only_commands {
    let CommandPermission::Allowed(allowed) = choice.permissions.commands() else {
      return Err(Message::new("appMcp.errors.narrowCommandsNotOffered"));
    };
    if allowed.entries().is_empty() || !enforced(PermissionOption::AllowedCommands) {
      return Err(Message::new("appMcp.errors.narrowCommandsNotOffered"));
    }
    let mut only = AllowedCommands::default();
    for command in commands {
      only = with_command(&only, command)?;
      if only
        .entries()
        .last()
        .is_some_and(|added| !allowed.entries().contains(added))
      {
        return Err(Message::new("appMcp.errors.commandNotInProfile").with("command", command.clone()));
      }
    }
    // Keeping every command of the profile narrows nothing, as the inspector's toggles have it.
    if only.entries().len() < allowed.entries().len() {
      permissions.commands = StepCommands::Only(only);
    }
  }
  if network_off {
    if choice.permissions.network() != NetworkAccess::On || !enforced(PermissionOption::NetworkOff) {
      return Err(Message::new("appMcp.errors.networkOffNotOffered"));
    }
    permissions.network = StepNetwork::Off;
  }
  Ok(permissions)
}

/// Whether an assigned executor and its narrowing can still be used. Unassigned drafts stay editable.
pub fn check_step(
  step: &orchester_core::resource::Step,
  assignables: &super::data::Assignables,
) -> Result<(), Message> {
  use orchester_core::resource::{RepositoryAccess, StepKind};
  match &step.kind {
    StepKind::Tool { tool: Some(id) } => {
      let choice = assignables
        .tool(*id)
        .ok_or_else(|| Message::new("appMcp.errors.toolNotPublished").with("tool", id.to_string()))?;
      if !choice.runnable {
        return Err(Message::new("appMcp.errors.toolNotRunnable").with("tool", choice.name.to_string()));
      }
    }
    StepKind::Agent {
      profile,
      repository,
      permissions: narrowing,
      ..
    } => {
      let choice = profile
        .map(|id| {
          assignables
            .profile(id)
            .ok_or_else(|| Message::new("appMcp.errors.profileNotPublished").with("profile", id.to_string()))
        })
        .transpose()?;
      // An unassigned starter is an incomplete draft; publication rejects missing executors.
      if *repository == RepositoryAccess::Write && choice.is_some_and(|choice| !can_write(choice)) {
        return Err(Message::new("appMcp.errors.stepCannotWrite"));
      }
      // Which step merges the base narrows nothing of the profile: any step may choose it.
      if inherit_profile(narrowing) != *narrowing || narrowing.interactive_tool_approvals {
        let choice = choice.ok_or_else(|| Message::new("appMcp.errors.narrowNeedsProfile"))?;
        let commands = match &narrowing.commands {
          StepCommands::AsProfile => None,
          StepCommands::Only(only) => Some(only.entries().iter().map(ToString::to_string).collect::<Vec<_>>()),
        };
        permissions(
          commands.as_deref(),
          narrowing.network == StepNetwork::Off,
          narrowing.interactive_tool_approvals,
          choice,
        )?;
      }
    }
    StepKind::Tool { tool: None } => {}
  }
  Ok(())
}
