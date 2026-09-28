//! MCP servers on the Permissions tab (04.F `VI4c6`, after the settings of 04.04 `T1rjv`, with the
//! switch `K8ItM`): a switch per project server says whether the profile's agents get it,
//! with where it runs, and why an agent would work without a chosen one (turned off, or a folder
//! Claude Code cannot start it in). The enforcement panel's MCP rows say what the chosen CLI enforces
//! for them (`orchester_core::permission::mcp`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::{
    AgentProvider,
    mcp::{McpServer, McpTransport},
  },
  permission::mcp::{McpCalls, McpLoading, mcp_support},
};

use super::{
  editor::{McpChoice, ProfileEditor},
  enforcement::row,
};
use crate::{
  navigation::route::{ConnectionTab, Page},
  screens::project::page::TOPBAR_BUTTON_HEIGHT,
  theme::{ExtraSpacing, ExtraTypography},
  ui::{button::ActionButton, icon::Icon, option_card::unavailable_reason, switch::Switch},
};

impl ProfileEditor {
  /// The project's MCP servers, each with a switch; or, without any, where to add one.
  pub(super) fn mcp_setting(&self, ctx: &Ctx, provider: Option<AgentProvider>) -> Element {
    let servers = &self.context.agents.mcp;
    let column = Column::new()
      .id("agent-mcp")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Inline.role())
      .overflow_visible()
      .child(Text::new(&ctx.t("agents.permissions.mcp.label")).variant(TypographyStyle::Label));
    if servers.is_empty() {
      let add = ActionButton::new(ctx.t("agents.permissions.mcp.add"))
        .icon(Icon::Plus)
        .id("agent-mcp-add")
        .in_form()
        .height(Dimension::Px(TOPBAR_BUTTON_HEIGHT))
        .link(
          self
            .context
            .nav
            .link(Page::Connections(ConnectionTab::McpServers, None)),
        )
        .render(ctx);
      return column
        .child(hint(ctx, "agents.permissions.mcp.none"))
        .child(Row::new().child(add))
        .into();
    }
    column
      .with_children(servers.iter().map(|server| self.server_row(ctx, server, provider)))
      .child(hint(ctx, "agents.permissions.mcp.hint"))
      .into()
  }

  /// A server's switch with where it runs, and why an agent would work without it.
  fn server_row(&self, ctx: &Ctx, server: &McpServer, provider: Option<AgentProvider>) -> Element {
    let chosen = self.mcp.get().0.contains(server.id());
    let detail = match server.transport() {
      McpTransport::Stdio(stdio) => ctx.t_args("connections.mcp.row.stdio", [("program", stdio.program().as_str())]),
      McpTransport::Http(http) => ctx.t_args("connections.mcp.row.http", [("host", http.url().host())]),
    };
    let (mcp, id) = (self.mcp.clone(), server.id());
    let switch = Switch::new(
      format!("agent-mcp-{}", server.name()),
      server.name().as_str().into(),
      chosen,
    )
    .emphasized()
    .detail(detail)
    .render(move || {
      mcp.update(|choice| {
        if let Ok(toggled) = choice.0.toggled(id) {
          *choice = McpChoice(toggled);
        }
      });
    });
    let unusable = self.unusable(ctx, server, provider);
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .overflow_visible()
      .child(switch)
      .with_children(unusable.map(|reason| unavailable_reason(&reason)))
      .into()
  }

  /// Why an agent of this profile would work without `server`, when it would.
  fn unusable(&self, ctx: &Ctx, server: &McpServer, provider: Option<AgentProvider>) -> Option<Arc<str>> {
    if !server.enabled() {
      return Some(ctx.t("agents.permissions.mcp.turnedOff"));
    }
    if usable(server, provider?) {
      return None;
    }
    let McpTransport::Stdio(stdio) = server.transport() else {
      return None;
    };
    let folder = stdio.working_directory()?;
    Some(ctx.t_args("agents.permissions.mcp.noFolder", [("folder", folder.as_str())]))
  }

  /// What the connection's CLI enforces for the chosen servers (`kXyN7`): only they load, how
  /// their tools are allowed, and that the servers work outside these permissions.
  pub(super) fn mcp_enforcement(&self, ctx: &Ctx, provider: AgentProvider) -> Vec<Element> {
    let chosen = self.mcp.get();
    // The chosen servers an agent gets: those the project has, that are on and that the CLI can start.
    let count = self
      .context
      .agents
      .mcp
      .iter()
      .filter(|server| chosen.0.contains(server.id()) && usable(server, provider))
      .count();
    let support = mcp_support(provider);
    let loaded = match count {
      0 if !chosen.0.is_empty() => ctx.t("agents.permissions.enforcement.mcp.noneLoad"),
      0 => ctx.t("agents.permissions.enforcement.mcp.none"),
      1 => ctx.t("agents.permissions.enforcement.mcp.onlyOne"),
      _ => ctx.t_args(
        "agents.permissions.enforcement.mcp.only",
        [("count", count.to_string().as_str())],
      ),
    };
    let loading = match (count, support.loading) {
      (0, _) if !chosen.0.is_empty() => ctx.t("agents.permissions.enforcement.mcp.noneLoadHow"),
      (0, _) => ctx.t("agents.permissions.enforcement.mcp.noneHow"),
      (_, McpLoading::CodexConfigOverrides) => ctx.t("agents.permissions.enforcement.mcp.codexLoading"),
      (_, McpLoading::ClaudeStrictConfig) => ctx.t("agents.permissions.enforcement.mcp.claudeLoading"),
    };
    let mut rows = vec![row(Icon::CircleCheck, &loaded, &loading)];
    if count == 0 {
      return rows;
    }
    rows.push(match support.calls {
      McpCalls::CodexNeverAsks => row(
        Icon::TriangleAlert,
        &ctx.t("agents.permissions.enforcement.mcp.codexCalls"),
        &ctx.t("agents.permissions.enforcement.mcp.codexCallsHow"),
      ),
      McpCalls::ClaudeAllowedTools => row(
        Icon::CircleCheck,
        &ctx.t("agents.permissions.enforcement.mcp.claudeCalls"),
        &ctx.t("agents.permissions.enforcement.mcp.claudeCallsHow"),
      ),
    });
    rows.push(row(
      Icon::Unplug,
      &ctx.t("agents.permissions.enforcement.mcp.outside"),
      &ctx.t("agents.permissions.enforcement.mcp.outsideHow"),
    ));
    rows
  }
}

/// Whether a `provider` agent gets `server` when its profile chooses it: the server is on, and the CLI
/// can start it where it is set to start.
fn usable(server: &McpServer, provider: AgentProvider) -> bool {
  let folder = match server.transport() {
    McpTransport::Stdio(stdio) => stdio.working_directory().is_some(),
    McpTransport::Http(_) => false,
  };
  server.enabled() && (!folder || mcp_support(provider).working_directory)
}

fn hint(ctx: &Ctx, key: &str) -> Element {
  Text::new(&ctx.t(key))
    .variant(ExtraTypography::Hint.role())
    .color(PaletteColor::TextMuted)
    .into()
}
