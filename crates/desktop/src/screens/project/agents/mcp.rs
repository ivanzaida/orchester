//! MCP servers on the Permissions tab (04.F `VI4c6`, after the settings of 04.04 `T1rjv`, with the
//! switch `K8ItM`): a switch per project server says whether the profile's agents get it,
//! with where it runs, and why an agent would work without a chosen one (turned off, or a folder
//! Claude Code cannot start it in). The enforcement panel's MCP rows say what the chosen CLI enforces
//! for them (`orchester_core::permission::mcp`), and, when the profile asks before MCP actions, which
//! servers run some tools without asking (no frame draws these rows; they follow the approval row).

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
  permission::mcp::starts_in_its_folder,
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

  /// What the connection's CLI enforces for the chosen servers (`kXyN7`): which load, how their
  /// tools are allowed, and that the servers work outside these permissions. Neither CLI gets the
  /// user's own servers.
  pub(super) fn mcp_enforcement(&self, ctx: &Ctx, provider: AgentProvider) -> Vec<Element> {
    let permissions = self.permissions.get();
    let chosen = self.mcp.get();
    // The chosen servers an agent gets: those the project has, that are on and that the CLI can start.
    let count = self
      .context
      .agents
      .mcp
      .iter()
      .filter(|server| chosen.0.contains(server.id()) && usable(server, provider))
      .count();
    if count == 0 {
      let (loaded, loading) = if chosen.0.is_empty() {
        (
          "agents.permissions.enforcement.mcp.none",
          "agents.permissions.enforcement.mcp.noneHow",
        )
      } else {
        (
          "agents.permissions.enforcement.mcp.noneLoad",
          "agents.permissions.enforcement.mcp.noneLoadHow",
        )
      };
      return vec![row(Icon::CircleCheck, &ctx.t(loaded), &ctx.t(loading))];
    }
    let exempt = if permissions.interactive_tool_approvals() {
      self.exempt_rows(ctx, provider)
    } else {
      Vec::new()
    };
    if provider == AgentProvider::CodexCli {
      let mut rows = app_server_enforcement(ctx, permissions.interactive_tool_approvals());
      rows.splice(2..2, exempt);
      return rows;
    }
    let loaded = if count == 1 {
      ctx.t("agents.permissions.enforcement.mcp.onlyOne")
    } else {
      ctx.t_args(
        "agents.permissions.enforcement.mcp.only",
        [("count", count.to_string().as_str())],
      )
    };
    let mut rows = vec![row(
      Icon::CircleCheck,
      &loaded,
      &ctx.t("agents.permissions.enforcement.mcp.claudeLoading"),
    )];
    // Asking before MCP actions leaves Claude Code's MCP tools to its permission prompt.
    rows.push(if permissions.interactive_tool_approvals() {
      row(
        Icon::CircleCheck,
        &ctx.t("agents.permissions.enforcement.mcp.claudeCallsAsk"),
        &ctx.t("agents.permissions.enforcement.mcp.claudeCallsAskHow"),
      )
    } else {
      row(
        Icon::CircleCheck,
        &ctx.t("agents.permissions.enforcement.mcp.claudeCalls"),
        &ctx.t("agents.permissions.enforcement.mcp.claudeCallsHow"),
      )
    });
    rows.extend(exempt);
    rows.push(row(
      Icon::Unplug,
      &ctx.t("agents.permissions.enforcement.mcp.outside"),
      &ctx.t("agents.permissions.enforcement.mcp.outsideHow"),
    ));
    rows
  }

  /// One row per chosen server an agent gets that runs tools without asking: how many, and how the CLI
  /// lets exactly those run.
  fn exempt_rows(&self, ctx: &Ctx, provider: AgentProvider) -> Vec<Element> {
    let servers = &self.context.agents.mcp;
    let chosen = self.mcp.get();
    chosen
      .0
      .exempt_tools(servers)
      .into_iter()
      .filter(|(name, _)| {
        servers
          .iter()
          .any(|server| server.name() == *name && usable(server, provider))
      })
      .map(|(server, tools)| {
        let key = if tools.len() == 1 {
          "agents.permissions.enforcement.mcp.exempt.one"
        } else {
          "agents.permissions.enforcement.mcp.exempt.other"
        };
        let count = tools.len().to_string();
        let title = ctx.t_args(key, [("server", server.as_str()), ("count", count.as_str())]);
        let how = match provider {
          AgentProvider::ClaudeCodeCli => "agents.permissions.enforcement.mcp.exemptClaudeHow",
          AgentProvider::CodexCli => "agents.permissions.enforcement.mcp.exemptCodexHow",
        };
        row(
          Icon::ListChecks,
          &title,
          &ctx.t_args(how, [("server", server.as_str())]),
        )
      })
      .collect()
  }
}

/// Whether a `provider` agent gets `server` when its profile chooses it: the server is on, and the CLI
/// can start it where it is set to start.
fn usable(server: &McpServer, provider: AgentProvider) -> bool {
  let folder = match server.transport() {
    McpTransport::Stdio(stdio) => stdio.working_directory().is_some(),
    McpTransport::Http(_) => false,
  };
  server.enabled() && (!folder || starts_in_its_folder(provider))
}

fn hint(ctx: &Ctx, key: &str) -> Element {
  Text::new(&ctx.t(key))
    .variant(ExtraTypography::Hint.role())
    .color(PaletteColor::TextMuted)
    .into()
}

/// Codex App Server's MCP rows when a chosen server runs: only the chosen servers run (the user's
/// own are turned off), and a tool call that asks for approval reaches a person only when the
/// profile asks before MCP actions; otherwise the request is denied without asking.
fn app_server_enforcement(ctx: &Ctx, tool_approvals: bool) -> Vec<Element> {
  let approvals = if tool_approvals {
    row(
      Icon::CircleCheck,
      &ctx.t("agents.permissions.toolApprovals"),
      &ctx.t("runs.toolApproval.scope"),
    )
  } else {
    row(
      Icon::TriangleAlert,
      &ctx.t("agents.permissions.toolApprovalsDisabled"),
      &ctx.t("agents.permissions.enforcement.mcp.codexDenied"),
    )
  };
  vec![
    row(
      Icon::CircleCheck,
      &ctx.t("agents.permissions.enforcement.mcp.codexChosen"),
      &ctx.t("agents.permissions.transport"),
    ),
    approvals,
    row(
      Icon::Unplug,
      &ctx.t("agents.permissions.enforcement.mcp.outside"),
      &ctx.t("agents.permissions.enforcement.mcp.outsideHow"),
    ),
  ]
}
