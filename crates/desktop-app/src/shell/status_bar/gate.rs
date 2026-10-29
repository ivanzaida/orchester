//! The sign-in gate's status bar (`Status bar` `UQ84A` in `Lpznn`, `Status bar · MCP item · sign-in required`
//! `e2DPHR`, `31 · Sign-in gate · after` `K8VUO`): only the app's MCP server, which refuses every tool while this
//! computer is signed out, says so in `warning` with a lock (*MCP 127.0.0.1:47821 · sign-in required*), and its card
//! on hover says what that means and the app's version. In a narrow window the address gives way and the words stay. A
//! server that is not listening says so as it does in the shell's bar, the version in its card. Nothing here opens
//! anything: behind the gate there is nowhere to go.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{Breakpoint, PaletteColor, RadiusSize, SpacingSize},
  },
  components::Row,
  layout::Alignment,
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_core::time::Timestamp;
use orchester_workspace::display;

use super::{
  STATUS_BAR_HEIGHT,
  item::{glyph, item, label},
  mcp, mcp_status,
};
use crate::{
  mcp::{McpKind, McpLink, McpStatus, ServerState, address},
  theme::ExtraColor,
  ui::{
    hover_card::{CardWords, Fact, FactValue, FactsCard, hover_card},
    icon::Icon,
  },
};

/// The gate's bar; the server has refused every tool since `since`.
pub fn gate(ctx: &mut Ctx, since: Timestamp) -> Element {
  let link = ctx
    .use_context::<McpLink>()
    .expect("the root provides the MCP server's link before mounting the gate");
  let status = mcp_status(ctx, &link);
  let narrow = ctx.breakpoint() < Some(Breakpoint::Xl);
  Row::new()
    .id("status-bar")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(STATUS_BAR_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .align_items(Alignment::Center)
    .child(Row::new().flex(1.0))
    .child(refusing(ctx, &status, (since, narrow)))
    .into()
}

/// *v0.3.0*: the app's version.
pub(super) fn version(ctx: &Ctx) -> String {
  ctx
    .t_args("shell.status.version", [("version", env!("CARGO_PKG_VERSION"))])
    .to_string()
}

/// The MCP item while signed out, with its card on hover.
fn refusing(ctx: &mut Ctx, status: &McpStatus, (since, narrow): (Timestamp, bool)) -> Element {
  let ServerState::Listening { port } = status.server else {
    let (words, color) = mcp::words(ctx, status);
    let anchor = item("status-mcp")
      .describe("state", mcp::state_name(&status.server))
      .child(glyph(Icon::RadioTower, color.clone()))
      .child(label(&words, color).id("status-mcp-text"));
    let card = FactsCard {
      id: "status-mcp-card".into(),
      dot: (PaletteColor::TextMuted, PaletteColor::TextPrimary),
      state: None,
      facts: vec![Fact {
        id: "version",
        label: ctx.t("shell.status.health.version").to_string(),
        value: FactValue::Words(version(ctx)),
      }],
      note: None,
      from_start: false,
      gives_way: None,
    };
    return hover_card(ctx, anchor, card);
  };
  let words = match (&status.kind, narrow) {
    (_, true) => ctx.t("shell.status.mcpLockedShort").to_string(),
    (McpKind::Installation, false) => ctx
      .t_args("shell.status.mcpLocked", [("address", address(port).to_string())])
      .to_string(),
    (McpKind::Qa { .. }, false) => ctx
      .t_args("shell.status.mcpLockedQa", [("port", port.to_string())])
      .to_string(),
    (McpKind::Development { .. }, false) => ctx
      .t_args("shell.status.mcpLockedDevelopment", [("port", port.to_string())])
      .to_string(),
  };
  // The `hover` fill while the pointer rests on it, as the shell's items take (`UaaqE`).
  let anchor = item("status-mcp")
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .describe("state", "sign-in-required")
    .child(glyph(Icon::Lock, PaletteColor::Warning))
    .child(label(&words, PaletteColor::Warning).id("status-mcp-text"));
  let card = CardWords {
    id: "status-mcp-card".into(),
    title: ctx.t("shell.mcp.locked.title").to_string(),
    dot: PaletteColor::Warning,
    state: ctx
      .t_args("shell.mcp.locked.since", [("time", display::clock(since))])
      .to_string(),
    body: ctx
      .t_args("shell.mcp.locked.body", [("address", address(port).to_string())])
      .to_string(),
    hint: ctx.t("shell.mcp.locked.hint").to_string(),
    note: Some(
      ctx
        .t_args("shell.mcp.locked.version", [("version", version(ctx))])
        .to_string(),
    ),
  };
  hover_card(ctx, anchor, card)
}
