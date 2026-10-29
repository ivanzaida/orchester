//! Whether an agent CLI can hand an Autopilot step's tool requests to Orchester, and the approval mode a step then
//! runs in (`docs/design/approval-modes.md` › Enforcement). Orchester answers every tool request itself; Autopilot
//! starts the CLI with its own permission prompt off, so that its allow rules, its read-only classifier and its MCP
//! rules decide nothing and each request reaches Orchester:
//!
//! - Claude Code ([`AutopilotMechanism::ClaudePermissionPrompt`], from [`CLAUDE_CODE_AUTOPILOT`]):
//!   `--permission-prompt-tool mcp__Orchester__approve` with `--settings {"permissions":{"ask":["Bash"]}}`, and no
//!   `Bash(<prefix>:*)` or `mcp__<server>` allow rules ([`super::claude::permission_args`]). The whole-tool ask rule is
//!   checked before `acceptEdits` and the read-only classifier, so every command, and every call of the profile's MCP
//!   servers, reaches the prompt tool. The same for Claude Code on an API provider (DeepSeek): the prompt is the CLI's,
//!   whatever model API it talks to.
//! - Codex App Server ([`AutopilotMechanism::CodexAppServer`], from [`CODEX_AUTOPILOT`]): the thread's granular
//!   approval policy with `sandbox_approval`, `rules` and `mcp_elicitations` on and `approvalsReviewer: "user"`, so
//!   every request Codex makes (an escalation out of its sandbox, a command an execpolicy rule asks about, an MCP call
//!   that needs approval) reaches Orchester; tools of a server that would run without asking are not exempt. Commands
//!   inside the sandbox ask nothing: the sandbox keeps their writes in the worktree and the writable folders.
//! - Native agents ([`AutopilotMechanism::Orchester`]): Orchester runs every tool call itself; nothing to turn off.
//!
//! A CLI older than the version it is known to work from, or one no check has found the version of, never runs
//! Autopilot: the step runs as Manual with [`ManualReason::CantEnforce`], and the profile's Connection enforcement
//! says why ([`AutopilotSupport`]).

use serde::{Deserialize, Serialize};

use super::{AgentPermissions, ApprovalMode};
use crate::connection::{AgentProvider, compare_versions};

/// The oldest Claude Code that hands every request to Orchester: `--permission-prompt-tool` and the whole-tool ask
/// rule of `--settings` were checked from this version on (`super::claude`, `crate::connection::claude_approval`).
pub const CLAUDE_CODE_AUTOPILOT: &str = "2.1.89";
/// The oldest Codex that hands every request to Orchester: App Server accepted the granular approval policy with
/// `approvalsReviewer: "user"` from this version on (`docs/evidence/orc41-interactive-approvals-2026-09-29.md`).
pub const CODEX_AUTOPILOT: &str = "0.159.0";

/// How a CLI hands an Autopilot step's requests to Orchester.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutopilotMechanism {
  /// `--permission-prompt-tool` with every command asking and no command or MCP allow rules.
  ClaudePermissionPrompt,
  /// App Server's granular approval policy, every kind of request on, the user as reviewer.
  CodexAppServer,
  /// A native agent: Orchester runs its tools.
  Orchester,
}

/// Why a CLI cannot hand its requests to Orchester.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "reason")]
pub enum CantEnforce {
  /// The connection's CLI is older than `required`.
  CliTooOld { installed: String, required: String },
  /// No check has found the connection's CLI version; `required` is the oldest that can.
  VersionUnknown { required: String },
}

/// Whether a connection can run Autopilot, for its Connection enforcement.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "support", content = "how")]
pub enum AutopilotSupport {
  Enforced(AutopilotMechanism),
  CantEnforce(CantEnforce),
}

/// Whether `provider`'s CLI of `version` (as its last check found it) can run Autopilot.
pub fn autopilot_support(provider: AgentProvider, version: Option<&str>) -> AutopilotSupport {
  let (mechanism, required) = match provider {
    AgentProvider::ClaudeCodeCli => (AutopilotMechanism::ClaudePermissionPrompt, CLAUDE_CODE_AUTOPILOT),
    AgentProvider::CodexCli => (AutopilotMechanism::CodexAppServer, CODEX_AUTOPILOT),
    AgentProvider::AnthropicApi | AgentProvider::OpenAiApi => {
      return AutopilotSupport::Enforced(AutopilotMechanism::Orchester);
    }
  };
  match version {
    None => AutopilotSupport::CantEnforce(CantEnforce::VersionUnknown {
      required: required.to_owned(),
    }),
    Some(installed) if compare_versions(Some(installed), Some(required)).is_lt() => {
      AutopilotSupport::CantEnforce(CantEnforce::CliTooOld {
        installed: installed.to_owned(),
        required: required.to_owned(),
      })
    }
    Some(_) => AutopilotSupport::Enforced(mechanism),
  }
}

/// Why a step runs as Manual.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "reason")]
pub enum ManualReason {
  /// The profile is set to Manual.
  Profile,
  /// The profile is set to Autopilot, but its CLI cannot hand its requests to Orchester.
  CantEnforce { why: CantEnforce },
}

/// The approval mode a step runs in on its connection.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "mode")]
pub enum EffectiveApprovalMode {
  Autopilot { how: AutopilotMechanism },
  Manual { why: ManualReason },
}

impl EffectiveApprovalMode {
  /// The mode of a step whose permissions chose `chosen`, on a connection with `support`.
  pub fn of(chosen: ApprovalMode, support: AutopilotSupport) -> Self {
    match (chosen, support) {
      (ApprovalMode::Manual, _) => Self::Manual {
        why: ManualReason::Profile,
      },
      (ApprovalMode::Autopilot, AutopilotSupport::Enforced(how)) => Self::Autopilot { how },
      (ApprovalMode::Autopilot, AutopilotSupport::CantEnforce(why)) => Self::Manual {
        why: ManualReason::CantEnforce { why },
      },
    }
  }

  pub const fn mode(&self) -> ApprovalMode {
    match self {
      Self::Autopilot { .. } => ApprovalMode::Autopilot,
      Self::Manual { .. } => ApprovalMode::Manual,
    }
  }
}

impl AgentPermissions {
  /// Whether these permissions hand every tool request to Orchester (Autopilot): the CLI's own allow rules decide
  /// nothing. Set only once the connection can ([`Self::with_effective_mode`]).
  pub const fn hands_requests_to_orchester(&self) -> bool {
    matches!(self.approval_mode(), ApprovalMode::Autopilot)
  }

  /// These permissions in the approval mode `effective` a step runs in: Autopilot only where its CLI hands its
  /// requests to Orchester, else Manual, as the profile's other permissions were.
  #[must_use]
  pub fn with_effective_mode(mut self, effective: &EffectiveApprovalMode) -> Self {
    self.set_approval_mode(effective.mode());
    self
  }
}
