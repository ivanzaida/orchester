//! The MCP item (`MCP port · status bar`): the address the embedded MCP server listens on, or
//! that it did not start. It opens nothing: the endpoint alone does not let a client in (the
//! token is in the discovery file), so the hint says where both are. The token is never shown.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  node::Element,
};

use super::{
  hint::{HintAnchor, Hints},
  item::{glyph, item, label, value},
};
use crate::{
  embedded_mcp::{self, McpStatus},
  ui::icon::Icon,
};

pub fn mcp_item(ctx: &Ctx, status: McpStatus, hints: &Hints) -> Element {
  let row = item("status-mcp");
  let (row, hint) = match status {
    McpStatus::Listening { port } => {
      let (endpoint, pid) = (embedded_mcp::endpoint(port), std::process::id().to_string());
      let row = row
        .describe("mcp", "listening")
        .child(glyph(Icon::Radio, PaletteColor::TextSecondary))
        .child(label(&ctx.t("statusBar.mcp.label"), PaletteColor::TextSecondary))
        .child(value(&embedded_mcp::address(port), PaletteColor::TextMuted));
      let args = [("endpoint", endpoint.as_str()), ("pid", pid.as_str())];
      (row, ctx.t_args("statusBar.hints.mcp", args))
    }
    McpStatus::Unavailable => {
      let row = row
        .describe("mcp", "unavailable")
        .child(glyph(Icon::ServerOff, PaletteColor::TextPrimary))
        .child(label(&ctx.t("statusBar.mcp.unavailable"), PaletteColor::TextPrimary));
      (row, ctx.t("statusBar.hints.mcpUnavailable"))
    }
  };
  hints.attach(row, HintAnchor::Mcp, hint).into()
}
