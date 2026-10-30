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
//!   unavailable. On Windows its sandbox does not turn network access off, so a host list cannot run there either. A
//!   command that needs more than the sandbox (network while it is off, a write outside the writable roots, an
//!   escalation the model requests) asks a person through App Server ([`OutsideCommands::SandboxApproval`]).
//! - Claude Code removes its edit tools for read-only and limits Bash to `--allowedTools` prefix rules, besides the
//!   commands it classifies as read-only, which it runs unlisted as Codex runs its known-safe ones ([`super::claude`]).
//!   Any other command asks a person through its permission prompt tool. It cannot confine an arbitrary command to the
//!   worktree (its own sandbox is not supported on Windows and is not used), and it can block its web tools but not
//!   network use by commands.
//! - Asking before commands or MCP actions routes each request to a person in Orchester: Codex through App Server,
//!   Claude Code through its permission prompt tool (`crate::connection::claude_approval`), where every command asks.
//!   Claude Code runs an approved command outside any sandbox, so in a read-only step the approval says it can change
//!   files.
//! - Native agents (`docs/architecture/native-agent.md` §5.4) are enforced by Orchester itself, alike on Windows and
//!   macOS: its file tools stay in the worktree (and the writable folders), its write tools are withheld from a
//!   read-only step, and every command outside the allowed list asks a person, except known-safe read-only ones once
//!   the list has an entry. It has no sandbox for commands, so it can neither confine any command to the worktree nor
//!   keep commands off the network. Of the native providers only the Anthropic API runs agents in this version
//!   ([`AgentProvider::runs_agents`]); the rows say how the others will be enforced.
//! - Extra writable folders ([`super::writable`]) widen what an agent that edits files may write. Codex adds them to
//!   its `workspace-write` sandbox (`sandbox_workspace_write.writable_roots`); Claude Code lets its edit tools write
//!   there (`--add-dir`), while its commands are not confined to any folder in the first place.
//! - Autopilot ([`super::EffectiveApprovalMode`]) hands every command to Orchester as asking before each one does,
//!   whatever the commands setting, which is Manual's; whether the CLI's version can is [`super::autopilot_support`]'s.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
  AgentPermissions, AllowedCommands, CommandPermission, LaunchPermissions, NetworkAccess, OutsideCommands,
  RepositoryAccess,
};
use crate::connection::{AgentProvider, invocation::Platform};

/// One of the three settings of [`AgentPermissions`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionSetting {
  Repository,
  Commands,
  Network,
}

/// A value a setting can take, without the command list or the host list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionOption {
  ReadOnly,
  EditWorktree,
  AskBeforeCommands,
  AllowedCommands,
  AnyCommand,
  NetworkOff,
  /// Only the hosts of a list (`NetworkAccess::Hosts`).
  NetworkHosts,
  NetworkOn,
}

impl PermissionOption {
  pub const fn setting(self) -> PermissionSetting {
    match self {
      Self::ReadOnly | Self::EditWorktree => PermissionSetting::Repository,
      Self::AskBeforeCommands | Self::AllowedCommands | Self::AnyCommand => PermissionSetting::Commands,
      Self::NetworkOff | Self::NetworkHosts | Self::NetworkOn => PermissionSetting::Network,
    }
  }

  /// The options of `setting` from the most restrictive to the widest. Asking before commands and a
  /// host list are left out: neither is a choice a reset falls back to (a reset has no list).
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

  pub const fn of_network(network: &NetworkAccess) -> Self {
    match network {
      NetworkAccess::Off => Self::NetworkOff,
      NetworkAccess::Hosts(_) => Self::NetworkHosts,
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
  /// Commands run inside the sandbox of the repository access; one that needs more asks a person
  /// (App Server, the approval policy's `sandbox_approval`).
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
  /// `--allowedTools "Edit(//<folder>/**)"` for the worktree and the extra writable folders, without
  /// `acceptEdits`: the step has allowed commands, and that mode would run `rm`, `sed` and the other
  /// file commands in any form ([`super::claude`]).
  ClaudeEditRules,
  /// `--add-dir <folder>`: the edit tools may also write there; commands were never confined.
  ClaudeAddDir,
  /// `--permission-prompt-tool` with a rule asking before every command: a person decides each one.
  ClaudePermissionPrompt,
  /// `--allowedTools "Bash(<prefix>:*)"`, one rule per command. Claude Code also runs the commands it
  /// classifies as read-only without a rule; any other command asks a person through the permission
  /// prompt tool. Without any entry every command asks, as with [`Self::ClaudePermissionPrompt`].
  ClaudeAllowedTools,
  /// WebFetch and WebSearch stay available.
  ClaudeWebTools,
  /// Network off was chosen but cannot be enforced: WebFetch and WebSearch are removed, commands
  /// keep network access (`claude::permission_args`).
  ClaudeWebToolsRemoved,
  /// Orchester offers a native agent no tool that writes; its file tools refuse writes by scope.
  NativeWriteToolsWithheld,
  /// Orchester's file tools read and write in the worktree only, `.git` and `.orchester` protected.
  NativeFileScope,
  /// Orchester's file tools may also write in the writable folders; commands were never confined.
  NativeFileScopeFolders,
  /// Orchester asks a person before every command.
  NativeCommandApprovals,
  /// Orchester runs the listed commands, unconfined, and asks a person about any other. Known-safe
  /// read-only commands run too once the list has an entry; with an empty list every command asks
  /// (`crate::native::command::CommandPolicy`).
  NativeCommandList,
  /// Commands the native agent runs reach the network as the OS lets them.
  NativeNetworkUnrestricted,
  /// A host list on Codex: the sandbox keeps commands off the network, as for network off, and
  /// `tools.web_search.allowed_domains=[…]` limits web search to the listed domains
  /// (`crate::connection::invocation::codex`).
  CodexSandboxNetworkHosts,
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
  /// The CLI cannot keep commands from reaching hosts outside a list on this platform.
  CannotLimitHosts,
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
///
/// A host list (`docs/evidence/orc-450-network-hosts-2026-10-30.md`, Codex CLI 0.159.2 and Claude Code 2.1.301):
/// neither CLI has a host allow-list for commands Orchester can pass. Codex runs a list where its sandbox keeps
/// commands off the network (macOS), with web search limited to the listed domains; Claude Code's commands keep
/// network access everywhere (its sandbox, with a domain list, is not supported on Windows and is not used), so a
/// list there, as on a native agent, refuses the step.
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
    (AgentProvider::CodexCli, O::NetworkHosts) => match platform {
      Platform::Windows => No(U::CannotLimitHosts),
      Platform::MacOs => Enforced(M::CodexSandboxNetworkHosts),
    },
    (AgentProvider::CodexCli, O::NetworkOn) => Enforced(M::CodexSandboxNetworkOn),
    (AgentProvider::ClaudeCodeCli, O::ReadOnly) => Enforced(M::ClaudeEditToolsRemoved),
    (AgentProvider::ClaudeCodeCli, O::EditWorktree) => Enforced(M::ClaudeAcceptEdits),
    (AgentProvider::ClaudeCodeCli, O::AllowedCommands) => Enforced(M::ClaudeAllowedTools),
    (AgentProvider::ClaudeCodeCli, O::AnyCommand) => No(U::CannotConfineCommands),
    (AgentProvider::ClaudeCodeCli, O::NetworkOff) => No(U::CannotBlockNetwork),
    (AgentProvider::ClaudeCodeCli, O::NetworkHosts) => No(U::CannotLimitHosts),
    (AgentProvider::ClaudeCodeCli, O::NetworkOn) => Enforced(M::ClaudeWebTools),
    (AgentProvider::AnthropicApi | AgentProvider::OpenAiApi, option) => native_support(option),
  }
}

/// The native rows (§5.4), the same on both platforms in this version.
const fn native_support(option: PermissionOption) -> Support {
  use Mechanism as M;
  use PermissionOption as O;
  match option {
    O::ReadOnly => Support::Enforced(M::NativeWriteToolsWithheld),
    O::EditWorktree => Support::Enforced(M::NativeFileScope),
    O::AskBeforeCommands => Support::Enforced(M::NativeCommandApprovals),
    O::AllowedCommands => Support::Enforced(M::NativeCommandList),
    O::AnyCommand => Support::Unavailable(Unavailable::CannotConfineCommands),
    O::NetworkOff => Support::Unavailable(Unavailable::CannotBlockNetwork),
    O::NetworkHosts => Support::Unavailable(Unavailable::CannotLimitHosts),
    O::NetworkOn => Support::Enforced(M::NativeNetworkUnrestricted),
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
  outside_commands: OutsideCommands,
}

impl EnforcedPermissions {
  /// The permissions the CLI runs with: the chosen ones, except network on when off cannot be
  /// enforced (see [`Self::network_stays_on`]). An invocation records these as its launch
  /// permissions.
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

  /// The listed commands Orchester itself runs without confining them to the worktree: a native
  /// agent's allowed list, chosen but not confined (§5.4). `None` when a CLI runs the commands, or
  /// when none is listed.
  pub fn unconfined_list(&self) -> Option<&AllowedCommands> {
    match (self.commands, self.permissions.commands()) {
      (Mechanism::NativeCommandList, CommandPermission::Allowed(list)) if !list.is_empty() => Some(list),
      _ => None,
    }
  }

  /// How the CLI asks a person about a command these permissions do not run by themselves: Codex
  /// whenever a command needs more than its sandbox, Claude Code about every command outside the
  /// allowed list (every command when the list is empty or it asks before commands).
  pub const fn outside_commands(&self) -> OutsideCommands {
    self.outside_commands
  }

  /// What an invocation that runs with these permissions records as its launch permissions.
  pub fn launch(&self) -> LaunchPermissions {
    LaunchPermissions::new(self.permissions.clone(), self.outside_commands)
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
    let (folders_mechanism, outside_commands) = traits(provider);
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
    let repository = match enforced(PermissionOption::of_repository(self.repository))? {
      Mechanism::ClaudeAcceptEdits if self.lists_commands() => Mechanism::ClaudeEditRules,
      mechanism => mechanism,
    };
    // Its commands setting must still be enforceable: Autopilot runs as Manual wherever the CLI cannot hand its
    // requests to Orchester (too old, or never checked), and Manual runs it. So preflight, saving a profile and a
    // run's permission change refuse what the runner's fallback could not start.
    let manual_commands = enforced(PermissionOption::of_commands(&self.commands))?;
    // Autopilot hands every command to Orchester, as asking before each one does; its list is Manual's.
    let commands = if self.hands_requests_to_orchester() {
      enforced(PermissionOption::AskBeforeCommands)?
    } else {
      manual_commands
    };
    let mut permissions = self.clone();
    let option = PermissionOption::of_network(&self.network);
    let (network, network_stays_on) = match support(provider, platform, option) {
      Support::Enforced(mechanism) => (mechanism, None),
      // Running a host list with network on would widen it; only network off falls back to on.
      Support::Unavailable(reason) if option == PermissionOption::NetworkHosts => {
        return Err(PermissionProblem::NotEnforceable { option, reason });
      }
      Support::Unavailable(reason) => {
        permissions.network = NetworkAccess::On;
        let mechanism = match provider {
          AgentProvider::ClaudeCodeCli => Mechanism::ClaudeWebToolsRemoved,
          AgentProvider::CodexCli | AgentProvider::AnthropicApi | AgentProvider::OpenAiApi => {
            enforced(PermissionOption::NetworkOn)?
          }
        };
        (mechanism, Some(reason))
      }
    };
    let writable_folders = permissions
      .granted_folders()
      .is_some_and(|folders| !folders.is_empty())
      .then_some(folders_mechanism);
    Ok(EnforcedPermissions {
      permissions,
      repository,
      commands,
      network,
      writable_folders,
      network_stays_on,
      outside_commands,
    })
  }

  /// Whether the CLI runs listed commands by rule: never in Autopilot, which gives it no rule.
  fn lists_commands(&self) -> bool {
    !self.hands_requests_to_orchester()
      && matches!(&self.commands, CommandPermission::Allowed(list) if !list.is_empty())
  }

  /// The network access a run on `provider` gets on `platform`: the chosen one, or on when off
  /// cannot be enforced there (as [`Self::enforced_on`] runs it).
  pub fn effective_network(&self, provider: AgentProvider, platform: Platform) -> NetworkAccess {
    match &self.network {
      NetworkAccess::Off if !support(provider, platform, PermissionOption::NetworkOff).is_enforced() => {
        NetworkAccess::On
      }
      network => network.clone(),
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
    // A host list is kept where it cannot be enforced, as a setting without any enforceable option is: resetting it
    // to network on would widen it, and enforcing it refuses it.
    let network = PermissionOption::of_network(&self.network);
    let hosts = network == PermissionOption::NetworkHosts;
    if let Some(option) = replacement(network).filter(|_| !hosts) {
      resets.push(network);
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

/// How a provider grants the extra writable folders and asks a person about the commands its
/// permissions do not run by themselves.
const fn traits(provider: AgentProvider) -> (Mechanism, OutsideCommands) {
  match provider {
    AgentProvider::CodexCli => (Mechanism::CodexWritableRoots, OutsideCommands::SandboxApproval),
    AgentProvider::ClaudeCodeCli => (Mechanism::ClaudeAddDir, OutsideCommands::PermissionPrompt),
    AgentProvider::AnthropicApi | AgentProvider::OpenAiApi => {
      (Mechanism::NativeFileScopeFolders, OutsideCommands::OrchesterAsks)
    }
  }
}
