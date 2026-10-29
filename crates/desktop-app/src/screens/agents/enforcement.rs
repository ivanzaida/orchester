//! What enforces a profile's permissions, in words: why an option cannot be chosen on a connection, the mechanism
//! behind each enforced setting, and the head's *Enforced by Claude Code* (01b).
use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  connection::{AgentConnection, AgentProvider, invocation::Platform},
  permission::{
    AgentPermissions, CommandPermission,
    capability::{Mechanism, PermissionOption, Support, Unavailable, support},
  },
};

use crate::screens::hub::parts::Tone;

pub fn unavailable(ctx: &Ctx, provider: Option<AgentProvider>, option: PermissionOption) -> Arc<str> {
  let key = match provider.map(|p| support(p, Platform::current(), option)) {
    Some(Support::Unavailable(Unavailable::CannotLimitCommands)) => "agents.cannotLimitCommands",
    Some(Support::Unavailable(Unavailable::CannotConfineCommands)) => "agents.cannotConfineCommands",
    Some(Support::Unavailable(Unavailable::CannotBlockNetwork)) => "agents.cannotBlockNetwork",
    _ => "agents.unavailable",
  };
  ctx.t(key)
}

pub fn mechanism(ctx: &Ctx, mechanism: Mechanism, permissions: &AgentPermissions) -> Arc<str> {
  let empty = match permissions.commands() {
    CommandPermission::Allowed(list) => list.is_empty(),
    CommandPermission::AskBefore => true,
    CommandPermission::AnyInWorktree => false,
  };
  let key = match mechanism {
    Mechanism::CodexReadOnlySandbox => "codexReadOnly",
    Mechanism::CodexWorkspaceWriteSandbox => "codexWorkspaceWrite",
    Mechanism::CodexSandboxedCommands => "codexCommands",
    Mechanism::CodexCommandApprovals => "codexCommandApprovals",
    Mechanism::CodexSandboxNetworkOff => "codexNetworkOff",
    Mechanism::CodexSandboxNetworkOn => "codexNetworkOn",
    Mechanism::CodexWritableRoots => "codexWritableRoots",
    Mechanism::ClaudeEditToolsRemoved if empty => "claudeEditToolsRemovedAsk",
    Mechanism::ClaudeEditToolsRemoved => "claudeEditToolsRemoved",
    Mechanism::ClaudeAcceptEdits => "claudeAcceptEdits",
    Mechanism::ClaudeEditRules => "claudeEditRules",
    Mechanism::ClaudeWebTools => "claudeWebTools",
    Mechanism::ClaudeWebToolsRemoved => "claudeWebToolsRemoved",
    Mechanism::ClaudeAddDir => "claudeAddDir",
    Mechanism::ClaudePermissionPrompt => "claudePermissionPrompt",
    Mechanism::NativeWriteToolsWithheld => "nativeWriteToolsWithheld",
    Mechanism::NativeFileScope => "nativeFileScope",
    Mechanism::NativeFileScopeFolders => "nativeFileScopeFolders",
    Mechanism::NativeCommandApprovals => "nativeCommandApprovals",
    Mechanism::NativeCommandList if empty => "nativeCommandListEmpty",
    Mechanism::NativeCommandList => "nativeCommandList",
    Mechanism::NativeNetworkUnrestricted => "nativeNetwork",
    Mechanism::ClaudeAllowedTools => {
      let count = match permissions.commands() {
        CommandPermission::Allowed(list) => list.entries().len(),
        _ => 0,
      };
      return ctx.t_args(
        if count == 0 {
          "agents.mechanism.claudeAllowedTools.none"
        } else {
          "agents.mechanism.claudeAllowedTools.other"
        },
        [("count", count.to_string())],
      );
    }
  };
  ctx.t(&format!("agents.mechanism.{key}"))
}

/// Whether the connection enforces every permission: *Enforced by …* in `success`, else why not in `warning`.
pub fn status(ctx: &Ctx, permissions: &AgentPermissions, connection: &AgentConnection) -> (Tone, Arc<str>) {
  let fallback = permissions.effective_network(connection.provider(), Platform::current()) != permissions.network();
  if fallback {
    return (Tone::Warning, ctx.t("agents.cannotBlockNetwork"));
  }
  if orchester_workspace::agents::permissions::fully_enforced(permissions, connection.provider()) {
    // A native agent's tools and commands are Orchester's own (`docs/architecture/native-agent.md` §5.4).
    let enforced = match connection.provider().is_native() {
      true => ctx.t("agents.enforcedByOrchester"),
      false => ctx.t_args("agents.enforced", [("provider", connection.name().as_str())]),
    };
    (Tone::Success, enforced)
  } else {
    (Tone::Warning, ctx.t("agents.notEnforcedState"))
  }
}
