//! How a pipeline step narrows its profile's permissions. A step can only choose values at or
//! below the profile's; the effective permissions of a step are profile ∩ step. The step's
//! repository access is its [`StepKind::Agent`](crate::resource::StepKind::Agent) `repository`.

use serde::{Deserialize, Serialize};

use super::{AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess};

/// The commands a step allows.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "commands")]
pub enum StepCommands {
  /// What the profile allows.
  #[default]
  AsProfile,
  /// Only these, and only as far as the profile allows them.
  Only(AllowedCommands),
}

/// The network access a step allows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepNetwork {
  /// What the profile allows.
  #[default]
  AsProfile,
  Off,
}

/// A step's narrowing of commands and network; plain data, every combination is valid.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StepPermissions {
  #[serde(default)]
  pub commands: StepCommands,
  #[serde(default)]
  pub network: StepNetwork,
  /// This agent step may pause for a person's decision on one Codex MCP or app tool call.
  /// The default keeps the existing non-interactive Codex policy for published pipelines.
  #[serde(default)]
  pub interactive_tool_approvals: bool,
}

impl StepPermissions {
  /// Nothing narrowed: the step runs with its profile's permissions (apart from repository access).
  pub fn is_as_profile(&self) -> bool {
    *self == Self::default()
  }
}

impl AgentPermissions {
  /// The permissions of a step that runs with these profile permissions, `repository` access and
  /// the `step` narrowing: each setting is the stricter of the two. Commands a step lists that the
  /// profile does not allow are dropped.
  pub fn narrowed(&self, repository: RepositoryAccess, step: &StepPermissions) -> Self {
    let repository = match (self.repository, repository) {
      (RepositoryAccess::Write, RepositoryAccess::Write) => RepositoryAccess::Write,
      _ => RepositoryAccess::ReadOnly,
    };
    let commands = match (&self.commands, &step.commands) {
      (profile, StepCommands::AsProfile) => profile.clone(),
      (CommandPermission::Allowed(profile), StepCommands::Only(step)) => {
        CommandPermission::Allowed(step.within(profile))
      }
      (CommandPermission::AnyInWorktree | CommandPermission::AskBefore, StepCommands::Only(step)) => {
        CommandPermission::Allowed(step.clone())
      }
    };
    let network = match (self.network, step.network) {
      (NetworkAccess::On, StepNetwork::AsProfile) => NetworkAccess::On,
      _ => NetworkAccess::Off,
    };
    Self::new(repository, commands, network).with_writable_folders(self.writable_folders.clone())
  }
}
