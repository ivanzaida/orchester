//! The list of connections (`Connection list` `Z7hYhu`): a header (`Connection list header` `Sznby`: 32 high,
//! its headings in 11/500 `text_muted`) over a row per connection (`Connection list row` `LoIZ8`: 56 high, 12
//! at its sides and between its cells): a 28 glyph tile, the name in 13/500 with what it is in 11.5
//! `text_muted` under it, its type and what uses it in 12 `text_secondary`, its status, and a chevron. The
//! inspected row takes the `hover` fill; a press on another opens its inspector. Rows are pointer-only.
//!
//! A narrow window has no header and compact rows (`Connection list row/compact` `a9O5lF` in `b3aoUZ`): 10 over
//! and under, 12 at the sides, the tile, then the name in 12.5/500 over its status and what it is, which wraps,
//! 3 apart, and the chevron; the type and what uses it are left to the inspector.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
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

/// The rows of `entries`, `selected` inspected, compact when `compact`; a press on a row opens its connection's
/// inspector with `open`.
pub(super) fn table(
  ctx: &Ctx,
  (listed, accounts): (&ProjectConnections, &Accounts),
  entries: &[Entry<'_>],
  (selected, open): (Option<ConnectionId>, &OpenConnection),
  compact: bool,
) -> Column {
  Column::new()
    .id("connections-list")
    .width(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .clip()
    .describe("compact", if compact { "true" } else { "false" })
    .with_children((!compact).then(|| heading(ctx)))
    .with_children(entries.iter().map(|entry| {
      let id = entry.id();
      let look = Look {
        selected: Some(id) == selected,
        compact,
      };
      row(ctx, (listed, accounts), *entry, look, open(id))
    }))
}

/// How a row is shown: inspected, and compact in a narrow window.
#[derive(Clone, Copy)]
struct Look {
  selected: bool,
  compact: bool,
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

fn row(ctx: &Ctx, listed: (&ProjectConnections, &Accounts), entry: Entry<'_>, look: Look, open: Open) -> Element {
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
  let row = if look.compact {
    compact_cells(ctx, &id, tile, &shape)
  } else {
    wide_cells(ctx, &id, tile, &shape)
  }
  .width(Dimension::Pct(100.0))
  .align_items(Alignment::Center)
  .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
  .child(icon(Icon::ChevronRight, IconSize::Md, PaletteColor::TextMuted));
  let row = if look.selected {
    row.background(ExtraColor::Hover.role())
  } else {
    row
  };
  opening(open)
    .id(id)
    .width(Dimension::Pct(100.0))
    .describe("selected", if look.selected { "true" } else { "false" })
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(row)
    .into()
}

/// A row's cells in a table with columns: the tile and the name over what the connection is, its type, what
/// uses it and its status, each in its column.
fn wide_cells(ctx: &Ctx, id: &str, tile: Row, shape: &Shape) -> Row {
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
  Row::new()
    .height(Dimension::Px(ROW_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Md))
    .spacing(SpacingSize::Md)
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
        .child(status_line(
          ctx,
          &format!("{id}-status"),
          shape.status,
          (IconSize::Base, TypographyStyle::Label),
        )),
    )
}

/// A compact row's cells (`a9O5lF`): the tile, then the name, the status and what the connection is stacked in
/// the room left, the name cut short rather than squeezed.
fn compact_cells(ctx: &Ctx, id: &str, tile: Row, shape: &Shape) -> Row {
  let text = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Fine.role())
    .child(
      Text::new(&shape.name)
        .id(format!("{id}-name"))
        .variant(ConnectionsText::CompactName.role())
        .width(Dimension::Pct(100.0))
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .child(status_line(
      ctx,
      &format!("{id}-status"),
      shape.status,
      (IconSize::Sm, ConnectionsText::CompactStatus.role()),
    ))
    .child(
      Text::new(&shape.line)
        .variant(ConnectionsText::CompactLine.role())
        .width(Dimension::Pct(100.0)),
    );
  Row::new()
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Loose.role())
    .child(tile)
    .child(text)
}
