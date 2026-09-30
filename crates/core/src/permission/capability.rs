//! Which permission options each agent CLI can enforce, per platform, and how. This is the one
//! place that knows it; the profile editor disables what is not enforceable, publishing and
//! preflight refuse it, and the invocation builders map what is.
//!
//! Evidence: `.tmp/cli-probe/REPORT.md` (Codex CLI 0.139 and Claude Code 2.1.89 on Windows 11).
//! macOS entries follow the CLIs' documented sandbox behaviour and were not probed; a capability
//! check per machine is to replace this table's platform guesses.
//!
//! - Codex enforces read-only and workspace-write through its sandbox (on Windows only with
//!   `windows.sandbox=elevated`). It has no way to deny every command outside a list, so *Allowed commands* is
//!   unavailable. On Windows its sandbox does not turn network access off.
//! - Claude Code removes its edit tools for read-only and limits Bash to `--allowedTools` prefix rules. It cannot
//!   confine an arbitrary command to the worktree (its own sandbox is not supported on Windows and is not used), and it
//!   can block its web tools but not network use by commands.
//! - Asking before commands or MCP actions routes each request to a person in Orchester: Codex through App Server,
//!   Claude Code through its permission prompt tool (`crate::connection::claude_approval`). Claude Code runs an
//!   approved command outside any sandbox, as it runs every command; the person sees the whole command first.
//! - Extra writable folders ([`super::writable`]) widen what an agent that edits files may write. Codex adds them to
//!   its `workspace-write` sandbox (`sandbox_workspace_write.writable_roots`); Claude Code lets its edit tools write
//!   there (`--add-dir`), while its commands are not confined to any folder in the first place.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess};
use crate::connection::{AgentProvider, invocation::Platform};

/// One of the three settings of [`AgentPermissions`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionSetting {
  Repository,
  Commands,
  Network,
}

/// A value a setting can take, without the command list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionOption {
  ReadOnly,
  EditWorktree,
  AskBeforeCommands,
  AllowedCommands,
  AnyCommand,
  NetworkOff,
  NetworkOn,
}

impl PermissionOption {
  pub const fn setting(self) -> PermissionSetting {
    match self {
      Self::ReadOnly | Self::EditWorktree => PermissionSetting::Repository,
      Self::AskBeforeCommands | Self::AllowedCommands | Self::AnyCommand => PermissionSetting::Commands,
      Self::NetworkOff | Self::NetworkOn => PermissionSetting::Network,
    }
  }

  /// The options of `setting` from the most restrictive to the widest. Asking before commands is
  /// left out: it is never the restrictive choice a reset falls back to.
  const fn by_restriction(setting: PermissionSetting) -> &'static [Self] {
    match setting {
      PermissionSetting::Repository => &[Self::ReadOnly, Self::EditWorktree],
      PermissionSetting::Commands => &[Self::AllowedCommands, Self::AnyCommand],
      PermissionSetting::Network => &[Self::NetworkOff, Self::NetworkOn],
    }
  }

  pub const fn of_repository(access: RepositoryAccess) -> Self {
    match access {
      RepositoryAccess::ReadOnly => Self::ReadOnly,
      RepositoryAccess::Write => Self::EditWorktree,
    }
  }

  pub const fn of_commands(commands: &CommandPermission) -> Self {
    match commands {
      CommandPermission::AskBefore => Self::AskBeforeCommands,
      CommandPermission::Allowed(_) => Self::AllowedCommands,
      CommandPermission::AnyInWorktree => Self::AnyCommand,
    }
  }

  pub const fn of_network(network: NetworkAccess) -> Self {
    match network {
      NetworkAccess::Off => Self::NetworkOff,
      NetworkAccess::On => Self::NetworkOn,
    }
  }
}

/// How a CLI enforces an option.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mechanism {
  /// `--sandbox read-only`.
  CodexReadOnlySandbox,
  /// `--sandbox workspace-write` (with `windows.sandbox=elevated` on Windows).
  CodexWorkspaceWriteSandbox,
  /// Commands run inside the sandbox of the repository access.
  CodexSandboxedCommands,
  /// Bidirectional App Server, one command decision without a persistent grant.
  CodexCommandApprovals,
  /// The sandbox's network access stays off.
  CodexSandboxNetworkOff,
  /// `sandbox_workspace_write.network_access=true`.
  CodexSandboxNetworkOn,
  /// `sandbox_workspace_write.writable_roots=[…]`: the sandbox also lets commands write there.
  CodexWritableRoots,
  /// `--disallowedTools Edit Write NotebookEdit`.
  ClaudeEditToolsRemoved,
  /// `--permission-mode acceptEdits`, edits in the working directory.
  ClaudeAcceptEdits,
  /// `--add-dir <folder>`: the edit tools may also write there; commands were never confined.
  ClaudeAddDir,
  /// `--permission-prompt-tool`: a person decides each command no rule allows, one call at a time.
  ClaudePermissionPrompt,
  /// `--allowedTools "Bash(<prefix>:*)"`, one rule per command.
  ClaudeAllowedTools,
  /// WebFetch and WebSearch stay available.
  ClaudeWebTools,
  /// Network off was chosen but cannot be enforced: WebFetch and WebSearch are removed, commands
  /// keep network access (`claude::permission_args`).
  ClaudeWebToolsRemoved,
}

/// Why a CLI cannot enforce an option.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unavailable {
  /// The CLI cannot deny every command outside a list.
  CannotLimitCommands,
  /// The CLI cannot keep arbitrary commands inside the worktree.
  CannotConfineCommands,
  /// The CLI cannot keep commands off the network on this platform.
  CannotBlockNetwork,
}

/// Whether a CLI enforces an option on a platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "support", content = "how")]
pub enum Support {
  Enforced(Mechanism),
  Unavailable(Unavailable),
}

impl Support {
  pub const fn is_enforced(self) -> bool {
    matches!(self, Self::Enforced(_))
  }
}

/// The capability table: whether `provider` enforces `option` on `platform`.
pub const fn support(provider: AgentProvider, platform: Platform, option: PermissionOption) -> Support {
  use Mechanism as M;
  use PermissionOption as O;
  use Support::{Enforced, Unavailable as No};
  use Unavailable as U;
  match (provider, option) {
    (AgentProvider::CodexCli, O::AskBeforeCommands) => Enforced(M::CodexCommandApprovals),
    (AgentProvider::ClaudeCodeCli, O::AskBeforeCommands) => Enforced(M::ClaudePermissionPrompt),
    (AgentProvider::CodexCli, O::ReadOnly) => Enforced(M::CodexReadOnlySandbox),
    (AgentProvider::CodexCli, O::EditWorktree) => Enforced(M::CodexWorkspaceWriteSandbox),
    (AgentProvider::CodexCli, O::AllowedCommands) => No(U::CannotLimitCommands),
    (AgentProvider::CodexCli, O::AnyCommand) => Enforced(M::CodexSandboxedCommands),
    (AgentProvider::CodexCli, O::NetworkOff) => match platform {
      Platform::Windows => No(U::CannotBlockNetwork),
      Platform::MacOs => Enforced(M::CodexSandboxNetworkOff),
    },
    (AgentProvider::CodexCli, O::NetworkOn) => Enforced(M::CodexSandboxNetworkOn),
    (AgentProvider::ClaudeCodeCli, O::ReadOnly) => Enforced(M::ClaudeEditToolsRemoved),
    (AgentProvider::ClaudeCodeCli, O::EditWorktree) => Enforced(M::ClaudeAcceptEdits),
    (AgentProvider::ClaudeCodeCli, O::AllowedCommands) => Enforced(M::ClaudeAllowedTools),
    (AgentProvider::ClaudeCodeCli, O::AnyCommand) => No(U::CannotConfineCommands),
    (AgentProvider::ClaudeCodeCli, O::NetworkOff) => No(U::CannotBlockNetwork),
    (AgentProvider::ClaudeCodeCli, O::NetworkOn) => Enforced(M::ClaudeWebTools),
  }
}

/// Why permissions cannot run on a connection. Publishing and preflight refuse them.
#[derive(Clone, Debug, Error, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "problem")]
pub enum PermissionProblem {
  /// No option of the setting is enforceable, so the profile cannot run on the connection.
  #[error("the connection cannot enforce any option of {setting:?}")]
  NoEnforceableOption { setting: PermissionSetting },
  /// The chosen option is not enforceable on the connection.
  #[error("the connection cannot enforce {option:?}: {reason:?}")]
  NotEnforceable {
    option: PermissionOption,
    reason: Unavailable,
  },
}

/// Permissions as a CLI runs them, with how each setting is enforced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnforcedPermissions {
  permissions: AgentPermissions,
  repository: Mechanism,
  commands: Mechanism,
  network: Mechanism,
  /// How the extra writable folders are granted, when the agent gets any.
  writable_folders: Option<Mechanism>,
  /// Network off was chosen but cannot be enforced, so network stays on.
  network_stays_on: Option<Unavailable>,
}

impl EnforcedPermissions {
  /// The permissions the CLI runs with: the chosen ones, except network on when off cannot be
  /// enforced (see [`Self::network_stays_on`]).
  pub const fn permissions(&self) -> &AgentPermissions {
    &self.permissions
  }

  pub const fn mechanism(&self, setting: PermissionSetting) -> Mechanism {
    match setting {
      PermissionSetting::Repository => self.repository,
      PermissionSetting::Commands => self.commands,
      PermissionSetting::Network => self.network,
    }
  }

  /// How the CLI grants the extra writable folders; `None` when the agent gets none (it only reads,
  /// or none are listed).
  pub const fn writable_folders(&self) -> Option<Mechanism> {
    self.writable_folders
  }

  /// Why network is on although off was chosen.
  pub const fn network_stays_on(&self) -> Option<Unavailable> {
    self.network_stays_on
  }
}

impl AgentPermissions {
  /// How `provider` enforces these permissions on `platform`, or why it cannot. Network off that
  /// cannot be enforced leaves network on, as the design decides (the profile stays usable and
  /// says so); any other option that cannot be enforced is refused, never run unenforced.
  pub fn enforced_on(
    &self,
    provider: AgentProvider,
    platform: Platform,
  ) -> Result<EnforcedPermissions, PermissionProblem> {
    for setting in [
      PermissionSetting::Repository,
      PermissionSetting::Commands,
      PermissionSetting::Network,
    ] {
      let any = PermissionOption::by_restriction(setting)
        .iter()
        .any(|option| support(provider, platform, *option).is_enforced());
      if !any {
        return Err(PermissionProblem::NoEnforceableOption { setting });
      }
    }
    let enforced = |option: PermissionOption| match support(provider, platform, option) {
      Support::Enforced(mechanism) => Ok(mechanism),
      Support::Unavailable(reason) => Err(PermissionProblem::NotEnforceable { option, reason }),
    };
    let repository = enforced(PermissionOption::of_repository(self.repository))?;
    let commands = enforced(PermissionOption::of_commands(&self.commands))?;
    let mut permissions = self.clone();
    let (network, network_stays_on) = match support(provider, platform, PermissionOption::of_network(self.network)) {
      Support::Enforced(mechanism) => (mechanism, None),
      Support::Unavailable(reason) => {
        permissions.network = NetworkAccess::On;
        let mechanism = match provider {
          AgentProvider::ClaudeCodeCli => Mechanism::ClaudeWebToolsRemoved,
          AgentProvider::CodexCli => enforced(PermissionOption::NetworkOn)?,
        };
        (mechanism, Some(reason))
      }
    };
    let writable_folders = permissions
      .granted_folders()
      .is_some_and(|folders| !folders.is_empty())
      .then_some(match provider {
        AgentProvider::CodexCli => Mechanism::CodexWritableRoots,
        AgentProvider::ClaudeCodeCli => Mechanism::ClaudeAddDir,
      });
    Ok(EnforcedPermissions {
      permissions,
      repository,
      commands,
      network,
      writable_folders,
      network_stays_on,
    })
  }

  /// The network access a run on `provider` gets on `platform`: the chosen one, or on when off
  /// cannot be enforced there (as [`Self::enforced_on`] runs it).
  pub const fn effective_network(&self, provider: AgentProvider, platform: Platform) -> NetworkAccess {
    match self.network {
      NetworkAccess::Off if !support(provider, platform, PermissionOption::NetworkOff).is_enforced() => {
        NetworkAccess::On
      }
      network => network,
    }
  }

  /// These permissions with every option `provider` cannot enforce on `platform` reset to the most
  /// restrictive one it can, and the options that were reset. Used when a profile's connection
  /// changes; the editor shows the resets before publishing. A setting without any enforceable
  /// option keeps its value (publishing then refuses it).
  pub fn fitted_to(&self, provider: AgentProvider, platform: Platform) -> (Self, Vec<PermissionOption>) {
    let mut fitted = self.clone();
    let mut resets = Vec::new();
    let replacement = |current: PermissionOption| {
      if support(provider, platform, current).is_enforced() {
        return None;
      }
      PermissionOption::by_restriction(current.setting())
        .iter()
        .copied()
        .find(|option| support(provider, platform, *option).is_enforced())
    };
    if let Some(option) = replacement(PermissionOption::of_repository(self.repository)) {
      resets.push(PermissionOption::of_repository(self.repository));
      fitted.repository = match option {
        PermissionOption::EditWorktree => RepositoryAccess::Write,
        _ => RepositoryAccess::ReadOnly,
      };
    }
    if let Some(option) = replacement(PermissionOption::of_commands(&self.commands)) {
      resets.push(PermissionOption::of_commands(&self.commands));
      fitted.commands = match option {
        PermissionOption::AnyCommand => CommandPermission::AnyInWorktree,
        _ => CommandPermission::Allowed(AllowedCommands::default()),
      };
    }
    if let Some(option) = replacement(PermissionOption::of_network(self.network)) {
      resets.push(PermissionOption::of_network(self.network));
      fitted.network = match option {
        PermissionOption::NetworkOn => NetworkAccess::On,
        _ => NetworkAccess::Off,
      };
    }
    (fitted, resets)
  }

  /// The most restrictive permissions `provider` can enforce on `platform`, for a new profile:
  /// read-only, no commands (or commands confined to the worktree), network off where it can be.
  pub fn most_restrictive(provider: AgentProvider, platform: Platform) -> Self {
    Self::new(
      RepositoryAccess::ReadOnly,
      CommandPermission::Allowed(AllowedCommands::default()),
      NetworkAccess::Off,
    )
    .fitted_to(provider, platform)
    .0
  }
}
