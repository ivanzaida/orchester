//! The groups of Connections (`Agent providers` `hXvgh`, `GitHub` `z6Ngb`, `MCP servers` `Jn0wB` in `05`): a row per
//! connection (`Claude Code · Work` `InVOF`: its 28 tile, its name over what it runs and who uses it, its status, and a
//! chevron; an MCP server's switch in place of its status), each opening the connection's page; Add provider and Add
//! MCP server at their groups' heads; and a group with nothing yet says so in a quiet row (`hJSDK`). Adding a GitHub
//! connection is not built yet: its empty row says why.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::{GitHubState, mcp::McpTransport},
  id::ConnectionId,
};
use orchester_workspace::connections::data::ProjectConnections;

use crate::{
  navigation::{Navigation, link::Open},
  project::ShownProject,
  routes::{ConnectionsView, NewConnection, ProjectPage, Route},
  screens::{
    connections::{
      data::{Entry, github_state},
      inspector::{Asks, Confirm, asked, mcp_switch, status_line},
      words::{GITHUB_GLYPH, agent_glyph, agent_line, connection_status, github_status, mcp_glyph, program_name},
      work::Work,
    },
    project_settings::kit::{EmptyRow, Glyph, Group, SettingRow, tile},
  },
  services::accounts::Accounts,
  theme::{ExtraSpacing, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    unavailable::reason_line,
  },
};

/// What a server's switch and question do, for the server they are about.
pub(super) type ListAsks = Arc<dyn Fn(ConnectionId) -> Asks + Send + Sync>;

/// The three groups, a question asked about an MCP server under its group.
pub(super) fn groups(
  ctx: &mut Ctx,
  project: &ShownProject,
  listed: &ProjectConnections,
  (accounts, asks): (&Accounts, &ListAsks),
  (running, asking): (Option<(ConnectionId, Work)>, Option<(ConnectionId, Confirm)>),
) -> Element {
  let navigation = Navigation::of(ctx);
  let id = project.open.project.id();
  let open = |connection: ConnectionId| {
    navigation.to(Route::Project(
      id,
      ProjectPage::Connections(ConnectionsView::of(connection)),
    ))
  };
  let add = |kind: NewConnection| navigation.to(Route::Project(id, ProjectPage::NewConnection(kind)));
  let name = project.open.project.name().as_str();
  let agents = agents_group(ctx, listed, accounts, (&open, add(NewConnection::AgentProvider)), name);
  let github = github_group(ctx, listed, &open);
  let mcp = mcp_group(
    ctx,
    listed,
    (asks, &open, add(NewConnection::McpServer)),
    (running, asking),
  );
  Column::new()
    .id("connections-list")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Roomy.role())
    .child(agents)
    .child(github)
    .child(mcp)
    .into()
}

/// A group's quiet add at its head (`Add provider` `f3gwhr`).
fn add_button(ctx: &Ctx, id: &str, key: &str, open: Open) -> ActionButton {
  ActionButton::new(id.to_owned(), ButtonKind::Ghost, ctx.t(key))
    .icon(Icon::Plus)
    .compact()
    .opens(open)
}

/// What a row ends with: its status, then the chevron that says it opens.
fn chevron() -> Element {
  icon(Icon::ChevronRight, IconSize::Md, PaletteColor::TextMuted).into()
}

/// How many profiles use a connection, after what it is.
fn with_usage(ctx: &Ctx, line: String, count: usize) -> String {
  let usage = match count {
    1 => ctx.t("connections.usage.one"),
    _ => ctx.t_args("connections.usage.other", [("count", count.to_string())]),
  };
  ctx
    .t_args("connections.row.usedBy", [("line", line), ("usage", usage.to_string())])
    .to_string()
}

fn agents_group(
  ctx: &mut Ctx,
  listed: &ProjectConnections,
  accounts: &Accounts,
  (open, add): (&impl Fn(ConnectionId) -> Open, Open),
  project: &str,
) -> Element {
  let group = Group::new("connections-agents", ctx.t("connections.groups.agentProviders")).end(add_button(
    ctx,
    "connections-add-agent",
    "connections.groups.addProvider",
    add.clone(),
  ));
  if listed.connections.is_empty() {
    return group
      .row(
        EmptyRow::new(
          "connections-agents-empty",
          Icon::Bot,
          ctx.t("connections.emptyProvidersTitle"),
          ctx.t_args("connections.emptyDetail", [("project", project)]),
        )
        .action(add_button(
          ctx,
          "connections-agents-empty-add",
          "connections.groups.add",
          add,
        )),
      )
      .into();
  }
  let rows: Vec<Element> = listed
    .connections
    .iter()
    .map(|connection| {
      // A connection on an API provider runs on its key, on no account (`y2qhJ`, `tnEKp`).
      let line = match super::super::account::named(ctx, accounts, connection.account(), connection.provider())
        .filter(|_| connection.api_provider().is_none())
      {
        Some(account) => ctx
          .t_args(
            "connections.row.withAccount",
            [("line", agent_line(ctx, connection)), ("account", account)],
          )
          .to_string(),
        None => agent_line(ctx, connection),
      };
      let line = with_usage(ctx, line, listed.used_by(connection.id()).count());
      let status = connection_status(connection, connection.status_with(&listed.secrets));
      row(
        ctx,
        Entry::Agent(connection),
        (agent_glyph(connection), line),
        vec![status_line(ctx, &format!("connection-row-{}-status", connection.id()), status, None).into()],
        open(connection.id()),
      )
    })
    .collect();
  group.rows(rows).into()
}

fn github_group(ctx: &mut Ctx, listed: &ProjectConnections, open: &impl Fn(ConnectionId) -> Open) -> Element {
  let group = Group::new("connections-github", ctx.t("connections.groups.github"));
  if listed.github.is_empty() {
    let reason = reason_line("connections-add-reason", &ctx.t("connections.github.addUnavailable"));
    return group
      .row(
        EmptyRow::new(
          "connections-github-empty",
          Icon::GitPullRequest,
          ctx.t("connections.github.emptyTitle"),
          ctx.t("connections.github.emptyShort"),
        )
        .action(reason),
      )
      .into();
  }
  let rows: Vec<Element> = listed
    .github
    .iter()
    .map(|connection| {
      let state = github_state(listed, connection);
      let line = match &state {
        GitHubState::SignedIn { account } => ctx.t_args(
          "connections.row.githubSignedIn",
          [("host", connection.host().as_str()), ("account", account.as_str())],
        ),
        _ => ctx.t_args("connections.row.github", [("host", connection.host().as_str())]),
      };
      let line = match listed.link_of(connection.id()) {
        Some(link) => ctx
          .t_args(
            "connections.row.githubFor",
            [("line", line.to_string()), ("repository", link.repo().to_string())],
          )
          .to_string(),
        None => line.to_string(),
      };
      let status = status_line(
        ctx,
        &format!("connection-row-{}-status", connection.id()),
        github_status(&state),
        None,
      );
      row(
        ctx,
        Entry::GitHub(connection),
        (GITHUB_GLYPH.into(), line),
        vec![status.into()],
        open(connection.id()),
      )
    })
    .collect();
  group.rows(rows).into()
}

fn mcp_group(
  ctx: &mut Ctx,
  listed: &ProjectConnections,
  (asks, open, add): (&ListAsks, &impl Fn(ConnectionId) -> Open, Open),
  (running, asking): (Option<(ConnectionId, Work)>, Option<(ConnectionId, Confirm)>),
) -> Element {
  let mut group = Group::new("connections-mcp", ctx.t("connections.groups.mcpServers")).end(add_button(
    ctx,
    "connections-add-mcp",
    "connections.groups.addMcp",
    add.clone(),
  ));
  if listed.mcp.is_empty() {
    return group
      .row(
        EmptyRow::new(
          "connections-mcp-empty",
          Icon::Server,
          ctx.t("connections.mcp.emptyTitle"),
          ctx.t("connections.mcp.emptyDetailShort"),
        )
        .action(add_button(
          ctx,
          "connections-mcp-empty-add",
          "connections.groups.add",
          add,
        )),
      )
      .into();
  }
  for server in listed.mcp.iter() {
    let users: Vec<String> = listed.used_by(server.id()).map(|used| used.role.to_string()).collect();
    let what = match server.transport() {
      McpTransport::Stdio(stdio) => {
        let mut words = vec![program_name(stdio.program().as_str()).to_owned()];
        words.extend(stdio.args().iter().map(|arg| arg.as_str().to_owned()));
        words.join(" ")
      }
      McpTransport::Http(http) => http.url().as_str().to_owned(),
    };
    let line = if users.is_empty() {
      ctx.t_args("connections.mcp.rowUnused", [("what", what)])
    } else {
      ctx.t_args(
        "connections.mcp.rowUsed",
        [("what", what), ("profiles", users.join(", "))],
      )
    };
    let id = server.id();
    let state = (
      running.filter(|(on, _)| *on == id).map(|(_, work)| work),
      asking.filter(|(on, _)| *on == id).map(|(_, confirm)| confirm),
    );
    let switch = mcp_switch(
      ctx,
      (&format!("connection-row-{id}-switch"), server),
      state,
      &asks(id),
      !users.is_empty(),
    );
    let row = row(
      ctx,
      Entry::Mcp(server),
      (mcp_glyph(server).into(), line.to_string()),
      vec![switch],
      open(id),
    );
    group = group.row(row);
    if let Some(confirm) = state.1 {
      group = group.after(asked(ctx, server, (confirm, &users), &asks(id)));
    }
  }
  group.into()
}

/// A connection's row: its tile, its name over `line`, `ends` and the chevron; it opens the connection's page.
fn row(ctx: &mut Ctx, entry: Entry<'_>, (glyph, line): (Glyph, String), mut ends: Vec<Element>, open: Open) -> Element {
  ends.push(chevron());
  SettingRow::new(format!("connection-row-{}", entry.id()), entry.name())
    .item()
    .lead(tile(ctx, glyph))
    .hint(line)
    .controls(ends)
    .opens(open)
    .render(ctx)
}
