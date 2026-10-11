//! Permission summary c7khPh and truthful connection mechanisms in VsylR.
use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::Row,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::{AgentProvider, invocation::Platform},
  permission::{
    AgentPermissions, CommandPermission, RepositoryAccess,
    capability::{Mechanism, PermissionOption, Support, Unavailable, support},
  },
};

use super::{fields::Fields, words};
use crate::{
  theme::{ControlHeight, IconSize},
  ui::icon::{Icon, icon},
};

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

pub fn summary(ctx: &Ctx, fields: &Fields, listed: &orchester_workspace::agents::data::ProjectAgents) -> Element {
  let permissions = fields.permissions.get().0;
  let provider = fields.connection(listed).map(|c| c.provider());
  let write = permissions.repository() == RepositoryAccess::Write;
  let mut row = Row::new()
    .id("agent-permission-summary")
    .min_height(Dimension::Px(ControlHeight::Sm.value()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(item(
      if write { Icon::PencilLine } else { Icon::Eye },
      &ctx.t(if write { "agents.writeTitle" } else { "agents.readOnly" }),
      PaletteColor::TextSecondary,
    ))
    .child(words::note(&ctx.t("agents.summarySeparator")))
    .child(item(
      Icon::Terminal,
      &words::commands(ctx, &permissions),
      PaletteColor::TextSecondary,
    ))
    .child(words::note(&ctx.t("agents.summarySeparator")))
    .child(item(
      Icon::Globe,
      &words::network(ctx, &permissions, provider),
      PaletteColor::TextSecondary,
    ));
  if let Some(connection) = fields.connection(listed) {
    let enforced = orchester_workspace::agents::permissions::fully_enforced(&permissions, connection.provider());
    let fallback = permissions.effective_network(connection.provider(), Platform::current()) != permissions.network();
    let text = if fallback {
      ctx.t("agents.cannotBlockNetwork")
    } else if enforced {
      ctx.t_args("agents.enforced", [("provider", connection.name().as_str())])
    } else {
      ctx.t("agents.notEnforced")
    };
    row = row.child(words::note(&ctx.t("agents.summarySeparator"))).child(item(
      if enforced { Icon::ShieldCheck } else { Icon::ShieldAlert },
      &text,
      if enforced {
        PaletteColor::Success
      } else {
        PaletteColor::Warning
      },
    ));
  }
  row.into()
}
fn item(glyph: Icon, text: &str, color: PaletteColor) -> Element {
  Row::new()
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, color.clone()))
    .child(words::note(text).color(color))
    .into()
}
