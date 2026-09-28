//! A connection in the project list (`sFtAz` as in `d9bo3`, `iYGKy`, `zh2zU`): icon, name and what it
//! runs, its type, how many agents or repositories use it, its status and a chevron. The selected
//! row is raised.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::{
  connection::{
    AgentConnection, GitHubConnection, GitHubState, REQUIRED_SCOPE,
    mcp::{McpServer, McpTransport},
  },
  id::ConnectionId,
};

use super::{
  super::super::github::status as github_status,
  status::{CHECKING, program_key, status_label, vendor_key},
};
use crate::{
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    github_mark::github_mark,
    icon::{Icon, IconSize, icon},
    pressable::{pressable, surface},
    status_label::compact_status_label as label,
  },
};

/// Row, icon surface and fixed column sizes (`d9bo3`, `lhuOK`, `JIsjZ`, `f6poW`).
const ROW_HEIGHT: f32 = 88.0;
const ICON_SURFACE: f32 = 36.0;
const TYPE_WIDTH: f32 = 76.0;
const USAGE_WIDTH: f32 = 58.0;

pub struct RowState {
  pub uses: usize,
  pub selected: bool,
  pub checking: bool,
}

/// What a row shows, whatever the kind of connection.
struct RowContent {
  id: ConnectionId,
  icon: Element,
  name: Arc<str>,
  description: Arc<str>,
  kind: Arc<str>,
  usage: Arc<str>,
  status: (Icon, Arc<str>),
}

pub fn connection_row(
  ctx: &Ctx,
  connection: &AgentConnection,
  state: &RowState,
  on_click: impl Fn() + Send + Sync + 'static,
) -> Element {
  let provider = connection.provider();
  let (vendor, program) = (ctx.t(vendor_key(provider)), ctx.t(program_key(provider)));
  let description = match connection.version() {
    Some(version) => ctx.t_args(
      "connections.row.description",
      [("vendor", &*vendor), ("program", &*program), ("version", version)],
    ),
    None => ctx.t_args(
      "connections.row.descriptionUnchecked",
      [("vendor", &*vendor), ("program", &*program)],
    ),
  };
  let usage_key = if state.uses == 1 {
    "connections.usage.one"
  } else {
    "connections.usage.other"
  };
  let (status_icon, status_key) = if state.checking {
    CHECKING
  } else {
    status_label(connection.status())
  };
  let content = RowContent {
    id: connection.id(),
    icon: icon(Icon::Bot, IconSize::Notice, PaletteColor::TextPrimary).into(),
    name: connection.name().as_str().into(),
    description,
    kind: ctx.t("connections.type.provider"),
    usage: ctx.t_args(usage_key, [("count", state.uses.to_string().as_str())]),
    status: (status_icon, ctx.t(status_key)),
  };
  row(content, state, on_click)
}

/// A GitHub connection (`iYGKy`): the host, `gh` and the account it is signed in to, and the
/// repository link that uses it (at most one).
pub fn github_row(
  ctx: &Ctx,
  connection: &GitHubConnection,
  github: &GitHubState,
  state: &RowState,
  on_click: impl Fn() + Send + Sync + 'static,
) -> Element {
  let host = connection.host().as_str();
  let description = match github {
    GitHubState::SignedIn { account }
    | GitHubState::MissingScope { account }
    | GitHubState::WrongAccount { active: account, .. } => {
      ctx.t_args("github.row.signedIn", [("host", host), ("account", account.as_str())])
    }
    GitHubState::NotSignedIn => ctx.t_args("github.row.notSignedIn", [("host", host)]),
    _ => ctx.t_args("github.row.description", [("host", host)]),
  };
  let usage_key = if state.uses == 1 {
    "github.row.repoOne"
  } else {
    "github.row.repoOther"
  };
  let (status_icon, status_key) = if state.checking {
    CHECKING
  } else {
    github_status::status_label(github)
  };
  let content = RowContent {
    id: connection.id(),
    icon: github_mark(ctx, PaletteColor::TextPrimary),
    name: connection.name().as_str().into(),
    description,
    kind: ctx.t("github.type"),
    usage: ctx.t_args(usage_key, [("count", state.uses.to_string().as_str())]),
    status: (status_icon, ctx.t_args(status_key, [("scope", REQUIRED_SCOPE)])),
  };
  row(content, state, on_click)
}

/// An MCP server (`zAFzk`, `zh2zU`, `YHRmd` in 04.D `QdLRd`): how the agent's CLI reaches it, the profiles that give it
/// to their agents, and whether it is on.
pub fn mcp_row(
  ctx: &Ctx,
  server: &McpServer,
  state: &RowState,
  on_click: impl Fn() + Send + Sync + 'static,
) -> Element {
  let usage_key = if state.uses == 1 {
    "connections.usage.one"
  } else {
    "connections.usage.other"
  };
  let (status_icon, status_key) = mcp_status(server);
  let content = RowContent {
    id: server.id(),
    icon: icon(Icon::Plug, IconSize::Notice, PaletteColor::TextPrimary).into(),
    name: server.name().as_str().into(),
    description: mcp_description(ctx, server),
    kind: ctx.t("connections.type.mcp"),
    usage: ctx.t_args(usage_key, [("count", state.uses.to_string().as_str())]),
    status: (status_icon, ctx.t(status_key)),
  };
  row(content, state, on_click)
}

/// How the agent's CLI reaches `server`, in one line: "Program · npx" or "Remote MCP · mcp.example.dev".
pub fn mcp_description(ctx: &Ctx, server: &McpServer) -> Arc<str> {
  match server.transport() {
    McpTransport::Stdio(stdio) => ctx.t_args("connections.mcp.row.stdio", [("program", stdio.program().as_str())]),
    McpTransport::Http(http) => ctx.t_args("connections.mcp.row.http", [("host", http.url().host())]),
  }
}

/// On, or turned off for every profile.
pub const fn mcp_status(server: &McpServer) -> (Icon, &'static str) {
  if server.enabled() {
    (Icon::CircleCheck, "connections.mcp.status.on")
  } else {
    (Icon::CircleMinus, "connections.mcp.status.off")
  }
}

fn row(content: RowContent, state: &RowState, on_click: impl Fn() + Send + Sync + 'static) -> Element {
  let identity = Column::new()
    .flex(1.0)
    .spacing(SpacingSize::Sm)
    .child(Text::new(&content.name).variant(TypographyStyle::Label).nowrap())
    .child(
      Text::new(&content.description)
        .variant(ExtraTypography::Footnote.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    );
  let row = surface()
    .id(Arc::<str>::from(format!("connection-{}", content.id)))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding_horizontal(ExtraSpacing::Stack.role())
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .background(if state.selected {
      PaletteColor::SurfaceRaised
    } else {
      ExtraColor::Surface.role()
    })
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(icon_surface(content.icon))
    .child(identity)
    .child(fixed_caption(&content.kind, TYPE_WIDTH))
    .child(fixed_caption(&content.usage, USAGE_WIDTH))
    .child(label(content.status.0, &content.status.1))
    .child(icon(Icon::ChevronRight, IconSize::Dismiss, PaletteColor::TextSecondary));
  // Rows are outside a form, so they are pointer-only: no Tab stop and no focus ring.
  if state.selected {
    return row.describe("state", "selected").into();
  }
  pressable(row, on_click)
    .hovered_style(Style::new().background(PaletteColor::AccentMuted))
    .into()
}

fn icon_surface(glyph: Element) -> Element {
  Row::new()
    .size(Dimension::Px(ICON_SURFACE), Dimension::Px(ICON_SURFACE))
    .background(PaletteColor::SurfaceRaised)
    .rounded(ExtraRadius::Notice.role())
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(glyph)
    .into()
}

fn fixed_caption(text: &str, width: f32) -> Element {
  Column::new()
    .width(Dimension::Px(width))
    .child(Text::new(text).variant(TypographyStyle::Caption).nowrap())
    .into()
}
