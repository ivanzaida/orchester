//! Connections with nothing yet (`07` `WYCDL`): *Connect an agent*, then a row per way to connect one — each CLI as
//! found on this computer's search path or chosen on the start page, with Add when it is, Check again when it is not
//! (only a copy found there, or that file, can be chosen), and a CLI on an API provider; GitHub not linked, unavailable
//! with why; and no MCP server yet with its Add.

use std::sync::Arc;

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::connection::AgentProvider;
use orchester_workspace::{background, connections::missing_cli::known_copies};

use crate::{
  navigation::{Navigation, link::Open},
  project::ShownProject,
  routes::{NewConnection, ProjectPage, Route},
  screens::{
    connections::words::provider_key,
    project_settings::kit::{EmptyRow, Glyph, Group, SettingRow, Tone, page_head, tile},
  },
  services::Services,
  theme::ExtraSpacing,
  ui::{
    agent_mark::AgentMark,
    button::{ActionButton, ButtonKind},
    icon::Icon,
    unavailable::reason_line,
  },
};

/// Which CLIs have a copy on the search path, as the page's future reads them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Found(Arc<Vec<(AgentProvider, bool)>>);

impl DevtoolsInspectable for Found {}

/// The CLIs a connection runs on, in the order the page lists them.
const CLIS: [AgentProvider; 2] = [AgentProvider::ClaudeCodeCli, AgentProvider::CodexCli];

pub(super) fn page(ctx: &mut Ctx, project: &ShownProject, recheck: &Signal<u64>) -> Element {
  let navigation = Navigation::of(ctx);
  let id = project.open.project.id();
  let add = |kind: NewConnection| navigation.to(Route::Project(id, ProjectPage::NewConnection(kind)));
  let workspace = Services::of(ctx).workspace;
  let found = ctx
    .future(recheck.get(), move |_| {
      let workspace = workspace.clone();
      async move {
        background::run(move || {
          Found(Arc::new(
            CLIS
              .into_iter()
              .map(|cli| (cli, !known_copies(&workspace, cli).copies.is_empty()))
              .collect(),
          ))
        })
        .await
      }
    })
    .state()
    .get();
  let mut rows: Vec<Element> = CLIS
    .into_iter()
    .map(|cli| {
      let on_path = found
        .data
        .as_ref()
        .and_then(|found| found.0.iter().find(|(listed, _)| *listed == cli).map(|(_, on)| *on));
      cli_row(ctx, cli, on_path, (add(NewConnection::AgentProvider), recheck))
    })
    .collect();
  rows.push(api_row(ctx, add(NewConnection::AgentProvider)));
  Column::new()
    .id("connections-empty")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Roomy.role())
    .child(page_head(
      "connections-empty-head",
      &ctx.t("connections.empty.title"),
      Some(&ctx.t("connections.empty.lead")),
    ))
    .child(Group::new("connections-agents", ctx.t("connections.groups.agentProviders")).rows(rows))
    .child(github_group(ctx))
    .child(mcp_group(ctx, add(NewConnection::McpServer)))
    .into()
}

/// A CLI on an API provider, with its Add.
fn api_row(ctx: &mut Ctx, add: Open) -> Element {
  SettingRow::new("connections-way-api", ctx.t("connections.empty.apiProvider"))
    .item()
    .lead(tile(ctx, Icon::Cloud))
    .hint(ctx.t("connections.empty.apiProviderDetail"))
    .control(
      ActionButton::new(
        "connections-way-api-add",
        ButtonKind::Secondary,
        ctx.t("connections.groups.add"),
      )
      .icon(Icon::Plus)
      .opens(add),
    )
    .render(ctx)
}

/// GitHub not linked, unavailable with why.
fn github_group(ctx: &Ctx) -> Group {
  let reason = reason_line("connections-add-reason", &ctx.t("connections.github.addUnavailable"));
  Group::new("connections-github", ctx.t("connections.groups.github")).row(
    EmptyRow::new(
      "connections-github-empty",
      Icon::GitPullRequest,
      ctx.t("connections.github.emptyTitle"),
      ctx.t("connections.github.emptyShort"),
    )
    .action(reason),
  )
}

/// No MCP server yet, with its Add.
fn mcp_group(ctx: &Ctx, add: Open) -> Group {
  Group::new("connections-mcp", ctx.t("connections.groups.mcpServers")).row(
    EmptyRow::new(
      "connections-mcp-empty",
      Icon::Server,
      ctx.t("connections.mcp.emptyTitle"),
      ctx.t("connections.mcp.emptyDetailShort"),
    )
    .action(
      ActionButton::new(
        "connections-mcp-empty-add",
        ButtonKind::Ghost,
        ctx.t("connections.groups.add"),
      )
      .icon(Icon::Plus)
      .compact()
      .opens(add),
    ),
  )
}

/// A CLI as found, its maker's mark leading: Add when a copy is on the search path, Check again when none is, nothing
/// while it is looked for.
fn cli_row(ctx: &mut Ctx, cli: AgentProvider, on_path: Option<bool>, (add, recheck): (Open, &Signal<u64>)) -> Element {
  let id = match cli {
    AgentProvider::ClaudeCodeCli => "connections-way-claude",
    _ => "connections-way-codex",
  };
  let row = SettingRow::new(id, ctx.t(provider_key(cli)))
    .item()
    .lead(tile(ctx, Glyph::Mark(AgentMark::of(cli))));
  let row = match on_path {
    None => row.hint(ctx.t("connections.empty.looking")),
    Some(true) => row.toned(ctx.t("connections.empty.found"), Tone::Success).control(
      ActionButton::new(
        format!("{id}-add"),
        ButtonKind::Primary,
        ctx.t("connections.groups.add"),
      )
      .icon(Icon::Plus)
      .opens(add),
    ),
    Some(false) => {
      let recheck = recheck.clone();
      row.hint(ctx.t("connections.empty.notFound")).control(
        ActionButton::new(
          format!("{id}-check"),
          ButtonKind::Ghost,
          ctx.t("connections.checkAgain"),
        )
        .icon(Icon::RefreshCw)
        .compact()
        .on_click(move || recheck.update(|count| *count += 1)),
      )
    }
  };
  row.render(ctx)
}
