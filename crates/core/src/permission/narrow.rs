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

/// A step's narrowing of commands and network, and whether it merges the base branch when that moves; plain data,
/// every combination is valid.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StepPermissions {
  #[serde(default)]
  pub commands: StepCommands,
  #[serde(default)]
  pub network: StepNetwork,
  /// This agent step may pause for a person's decision on one Codex MCP or app tool call.
  /// The default keeps the existing non-interactive Codex policy for published pipelines.
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub interactive_tool_approvals: bool,
  /// The pipeline's own choice of whether this step's agent is told to merge the base branch when it moves (ORC-65);
  /// `None`, as in pipelines saved before it existed, follows the default rule
  /// ([`crate::resource::Pipeline::takes_base_moves`]).
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub takes_base_moves: Option<bool>,
}

impl StepPermissions {
  /// Nothing narrowed and nothing chosen: the step runs with its profile's permissions (apart from repository access)
  /// and follows the default rule for base moves.
  pub fn is_as_profile(&self) -> bool {
    *self == Self::default()
  }
}

impl AgentPermissions {
  /// The permissions of a step that runs with these profile permissions, `repository` access and
  /// the `step` narrowing: each setting is the stricter of the two. Commands a step lists that the
  /// profile does not allow are dropped, and a profile that asks a person about commands keeps asking.
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
      (CommandPermission::AnyInWorktree, StepCommands::Only(step)) => CommandPermission::Allowed(step.clone()),
      // A list would let its commands run without asking where the profile asks a person; the step keeps
      // asking. The asking mode has no list of its own, so such a step does not start at all: preflight
      // refuses it (`PreflightProblem::CommandListNotOffered`) until a person updates the step.
      (CommandPermission::AskBefore, StepCommands::Only(_)) => CommandPermission::AskBefore,
    };
    let network = match (self.network, step.network) {
      (NetworkAccess::On, StepNetwork::AsProfile) => NetworkAccess::On,
      _ => NetworkAccess::Off,
    };
    let mut effective = Self::new(repository, commands, network).with_writable_folders(self.writable_folders.clone());
    effective.set_interactive_tool_approvals(self.interactive_tool_approvals() || step.interactive_tool_approvals);
    // A step does not narrow who answers: the profile's auto-approval and approval mode hold for each of its steps.
    effective.set_auto_approve(self.auto_approve);
    effective.set_approval_mode(self.approval_mode);
    effective
  }
}
