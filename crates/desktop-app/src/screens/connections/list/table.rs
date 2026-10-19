//! The list of connections (`Connection list` `Z7hYhu`): a header (`Connection list header` `Sznby`: 32 high,
//! its headings in 11/500 `text_muted`) over a row per connection (`Connection list row` `LoIZ8`: 56 high, 12
//! at its sides and between its cells): a 28 glyph tile, the name in 13/500 with what it is in 11.5
//! `text_muted` under it, its type and what uses it in 12 `text_secondary`, its status, and a chevron. The
//! inspected row takes the `hover` fill; a press on another opens its inspector. Rows are pointer-only.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{connection::GitHubState, id::ConnectionId};
use orchester_workspace::connections::data::ProjectConnections;

use crate::{
  navigation::link::Open,
  screens::connections::{
    data::{Entry, github_state},
    inspector::status_line,
    words::{
      GITHUB_GLYPH, Shown, agent_glyph, agent_line, connection_status, github_status, mcp_glyph, mcp_status,
      program_name,
    },
  },
  services::accounts::Accounts,
  theme::{ConnectionsText, ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::opening,
    table::HEADER_HEIGHT,
  },
};

/// A row's height (`LoIZ8`), its glyph tile (`Glyph` `UtwwL`) and the widths of its cells.
const ROW_HEIGHT: f32 = 56.0;
const GLYPH_TILE: f32 = 28.0;
const TYPE_WIDTH: f32 = 88.0;
const USED_WIDTH: f32 = 80.0;
const STATUS_WIDTH: f32 = 132.0;
const OPEN_WIDTH: f32 = 14.0;

/// What opens a connection's inspector.
pub(super) type OpenConnection = std::sync::Arc<dyn Fn(ConnectionId) -> Open + Send + Sync>;

/// The rows of `entries`, `selected` inspected; a press on a row opens its connection's inspector with `open`.
pub(super) fn table(
  ctx: &Ctx,
  (listed, accounts): (&ProjectConnections, &Accounts),
  entries: &[Entry<'_>],
  (selected, open): (Option<ConnectionId>, &OpenConnection),
) -> Column {
  Column::new()
    .id("connections-list")
    .width(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .clip()
    .child(heading(ctx))
    .with_children(entries.iter().map(|entry| {
      let id = entry.id();
      row(ctx, (listed, accounts), *entry, Some(id) == selected, open(id))
    }))
}

fn heading(ctx: &Ctx) -> Row {
  let cell = |key: &str, width: Option<f32>| {
    let text = Text::new(&ctx.t(key)).variant(ConnectionsText::ColumnHeading.role());
    match width {
      Some(width) => text.width(Dimension::Px(width)).flex_shrink(0.0),
      None => text.flex(1.0),
    }
  };
  Row::new()
    .id("connections-list-header")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEADER_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Md))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(cell("connections.columns.connection", None))
    .child(cell("connections.columns.type", Some(TYPE_WIDTH)))
    .child(cell("connections.columns.usedBy", Some(USED_WIDTH)))
    .child(cell("connections.columns.status", Some(STATUS_WIDTH)))
    .child(Row::new().width(Dimension::Px(OPEN_WIDTH)))
}

/// What a row shows of its connection.
struct Shape {
  glyph: Icon,
  name: String,
  line: String,
  kind: std::sync::Arc<str>,
  used: std::sync::Arc<str>,
  status: Shown,
}

fn shape(ctx: &Ctx, (listed, accounts): (&ProjectConnections, &Accounts), entry: Entry<'_>) -> Shape {
  let agents = |count: usize| match count {
    1 => ctx.t("connections.usage.one"),
    _ => ctx.t_args("connections.usage.other", [("count", count.to_string())]),
  };
  match entry {
    Entry::Agent(connection) => Shape {
      glyph: agent_glyph(connection.provider()),
      name: connection.name().as_str().to_owned(),
      // A connection on an API provider runs on its key, on no account (`y2qhJ`, `tnEKp`).
      line: match super::super::account::named(ctx, accounts, connection.account(), connection.provider())
        .filter(|_| connection.api_provider().is_none())
      {
        Some(account) => ctx
          .t_args(
            "connections.row.withAccount",
            [("line", agent_line(ctx, connection)), ("account", account)],
          )
          .to_string(),
        None => agent_line(ctx, connection),
      },
      kind: ctx.t(if connection.provider().is_native() {
        "connections.type.native"
      } else {
        "connections.type.provider"
      }),
      used: agents(listed.used_by(connection.id()).count()),
      status: connection_status(connection, connection.status_with(&listed.secrets)),
    },
    Entry::GitHub(connection) => {
      let state = github_state(listed, connection);
      let repos = usize::from(listed.link_of(connection.id()).is_some());
      Shape {
        glyph: GITHUB_GLYPH,
        name: connection.name().as_str().to_owned(),
        line: match &state {
          GitHubState::SignedIn { account } => ctx.t_args(
            "connections.row.githubSignedIn",
            [("host", connection.host().as_str()), ("account", account.as_str())],
          ),
          _ => ctx.t_args("connections.row.github", [("host", connection.host().as_str())]),
        }
        .to_string(),
        kind: ctx.t("connections.type.github"),
        used: match repos {
          1 => ctx.t("connections.repos.one"),
          _ => ctx.t_args("connections.repos.other", [("count", repos.to_string())]),
        },
        status: github_status(&state),
      }
    }
    Entry::Mcp(server) => Shape {
      glyph: mcp_glyph(server),
      name: server.name().as_str().to_owned(),
      line: mcp_line(ctx, server),
      kind: ctx.t("connections.type.mcp"),
      used: agents(listed.used_by(server.id()).count()),
      status: mcp_status(server.enabled()),
    },
  }
}

fn mcp_line(ctx: &Ctx, server: &orchester_core::connection::mcp::McpServer) -> String {
  use orchester_core::connection::mcp::McpTransport;
  match server.transport() {
    McpTransport::Stdio(stdio) => ctx
      .t_args(
        "connections.mcp.row.stdio",
        [("program", program_name(stdio.program().as_str()))],
      )
      .to_string(),
    McpTransport::Http(http) => ctx
      .t_args("connections.mcp.row.http", [("host", http.url().host())])
      .to_string(),
  }
}

fn row(ctx: &Ctx, listed: (&ProjectConnections, &Accounts), entry: Entry<'_>, selected: bool, open: Open) -> Element {
  let shape = shape(ctx, listed, entry);
  let id = format!("connection-row-{}", entry.id());
  let tile = Row::new()
    .width(Dimension::Px(GLYPH_TILE))
    .height(Dimension::Px(GLYPH_TILE))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(icon(shape.glyph, IconSize::Md, PaletteColor::TextSecondary));
  let text = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(
      Text::new(&shape.name)
        .id(format!("{id}-name"))
        .variant(ExtraTypography::Strong.role())
        .nowrap(),
    )
    .child(Text::new(&shape.line).variant(ExtraTypography::Note.role()).nowrap());
  let cell = |text: &str, width: f32| {
    Text::new(text)
      .variant(TypographyStyle::Description)
      .width(Dimension::Px(width))
      .flex_shrink(0.0)
      .nowrap()
  };
  let row = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Md))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Row::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Loose.role())
        .align_items(Alignment::Center)
        .child(tile)
        .child(text),
    )
    .child(cell(&shape.kind, TYPE_WIDTH))
    .child(cell(&shape.used, USED_WIDTH))
    .child(
      Row::new()
        .width(Dimension::Px(STATUS_WIDTH))
        .flex_shrink(0.0)
        .child(status_line(ctx, &format!("{id}-status"), shape.status, IconSize::Base)),
    )
    .child(icon(Icon::ChevronRight, IconSize::Md, PaletteColor::TextMuted));
  let row = if selected {
    row.background(ExtraColor::Hover.role())
  } else {
    row
  };
  opening(open)
    .id(id)
    .width(Dimension::Pct(100.0))
    .describe("selected", if selected { "true" } else { "false" })
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(row)
    .into()
}
