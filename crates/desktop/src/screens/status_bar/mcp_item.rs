//! The MCP item (`MCP port · status bar`): the address Orchester's own MCP server listens on, through
//! which automation controls the app's data, or that it did not start. A click opens its popover with
//! *Let automation answer approvals* ([`super::mcp_popover`]). The endpoint alone does not let a client
//! in (the token is in the discovery file), so the hint says where both are, and where lurq's server,
//! which inspects the window, listens. The token is never shown.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  core::Signal,
  node::Element,
};

use super::{
  hint::{HintAnchor, Hints},
  item::{clickable, glyph, item, label, value},
};
use crate::{
  app_mcp::{self, AppMcpStatus},
  embedded_mcp::{self, McpStatus},
  ui::icon::Icon,
};

/// What the item shows: both servers' status, and whether its popover is open.
#[derive(Clone)]
pub struct McpItem {
  /// lurq's server, for looking at the window.
  pub window: McpStatus,
  /// Orchester's own server, for automation.
  pub app: AppMcpStatus,
  pub popover: Signal<bool>,
}

pub fn mcp_item(ctx: &Ctx, mcp: &McpItem, hints: &Hints) -> Element {
  let row = item("status-mcp");
  let pid = std::process::id().to_string();
  let (row, hint) = match mcp.app {
    AppMcpStatus::Listening { port } => {
      let endpoint = app_mcp::endpoint(port);
      let row = row
        .describe("mcp", "listening")
        .child(glyph(Icon::Radio, PaletteColor::TextSecondary))
        .child(label(&ctx.t("statusBar.mcp.label"), PaletteColor::TextSecondary))
        .child(value(&app_mcp::address(port), PaletteColor::TextMuted));
      let hint = match mcp.window {
        McpStatus::Listening { port: window } => {
          let window = embedded_mcp::endpoint(window);
          let args = [
            ("endpoint", endpoint.as_str()),
            ("pid", pid.as_str()),
            ("window", window.as_str()),
          ];
          ctx.t_args("statusBar.hints.mcp", args)
        }
        McpStatus::Unavailable => ctx.t_args(
          "statusBar.hints.mcpWithoutWindow",
          [("endpoint", endpoint.as_str()), ("pid", pid.as_str())],
        ),
      };
      let popover = mcp.popover.clone();
      (clickable(row, move || popover.update(|open| *open = !*open)), hint)
    }
    AppMcpStatus::Unavailable => {
      let row = row
        .describe("mcp", "unavailable")
        .child(glyph(Icon::ServerOff, PaletteColor::TextPrimary))
        .child(label(&ctx.t("statusBar.mcp.unavailable"), PaletteColor::TextPrimary));
      (row, ctx.t("statusBar.hints.mcpUnavailable"))
    }
  };
  hints.attach(row, HintAnchor::Mcp, hint).into()
}
