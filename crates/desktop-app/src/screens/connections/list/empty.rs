//! Connections with nothing yet (`07` `WYCDL`): *Connect an agent*, then a row per way to connect one — each CLI as
//! found on this computer's search path or chosen on the start page, saying which, with Add when it is, Check again
//! when it is not (only a copy found there, or that file, can be chosen), and a CLI on an API provider; GitHub not
//! linked, unavailable with why; and no MCP server yet with its Add.

use std::sync::Arc;

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::connection::AgentProvider;
use orchester_workspace::{
  background,
  connections::missing_cli::{KnownCopies, known_copies},
};

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

/// Where a CLI has a copy a new connection can run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CliCopy {
  OnSearchPath,
  /// Only the file chosen on the start page, which is on no search path.
  Chosen,
  Absent,
}

impl CliCopy {
  fn of(known: &KnownCopies) -> Self {
    match (known.on_search_path, known.copies.len()) {
      (0, 0) => Self::Absent,
      (0, _) => Self::Chosen,
      _ => Self::OnSearchPath,
    }
  }
}

/// Where each CLI has a copy, as the page's future reads them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Found(Arc<Vec<(AgentProvider, CliCopy)>>);

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
              .map(|cli| (cli, CliCopy::of(&known_copies(&workspace, cli))))
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
      let copy = found
        .data
        .as_ref()
        .and_then(|found| found.0.iter().find(|(listed, _)| *listed == cli).map(|(_, copy)| *copy));
      cli_row(ctx, cli, copy, (add(NewConnection::AgentProvider), recheck))
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

/// A CLI as found, its maker's mark leading: Add when it has a copy to run, saying where, Check again when it has none,
/// nothing while it is looked for.
fn cli_row(ctx: &mut Ctx, cli: AgentProvider, copy: Option<CliCopy>, (add, recheck): (Open, &Signal<u64>)) -> Element {
  let id = match cli {
    AgentProvider::ClaudeCodeCli => "connections-way-claude",
    _ => "connections-way-codex",
  };
  let row = SettingRow::new(id, ctx.t(provider_key(cli)))
    .item()
    .lead(tile(ctx, Glyph::Mark(AgentMark::of(cli))));
  let add_button = |ctx: &Ctx| {
    ActionButton::new(
      format!("{id}-add"),
      ButtonKind::Primary,
      ctx.t("connections.groups.add"),
    )
    .icon(Icon::Plus)
    .opens(add)
  };
  let row = match copy {
    None => row.hint(ctx.t("connections.empty.looking")),
    Some(CliCopy::OnSearchPath) => row
      .toned(ctx.t("connections.empty.found"), Tone::Success)
      .control(add_button(ctx)),
    Some(CliCopy::Chosen) => row
      .toned(ctx.t("connections.empty.foundChosen"), Tone::Success)
      .control(add_button(ctx)),
    Some(CliCopy::Absent) => {
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
