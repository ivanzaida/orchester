//! The app's MCP server in the status bar (`Status item` `wnnsD`; its states and popover `QqBCj`): the address it
//! listens on (*MCP 127.0.0.1:47821*), or that its port is taken, that it is off or not running, in `danger` when
//! agents cannot reach it; a QA copy or a development build names its port only (*MCP :53122 · QA*). It never
//! gives way. A click opens the popover over it: the server's state, the address or the discovery file with Copy,
//! the agents connected, whether agents may control the window, and *Open Automation settings*. It names where the
//! token is kept, never the token.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, TypographyStyle},
  },
  clipboard::copy_to_clipboard,
  components::{Column, Row, Spacer, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  facts::{BarAction, Dispatch},
  item::{dot, glyph, label, pressed},
};
use crate::{
  mcp::{McpKind, McpStatus, ServerState, WindowControlSource, address, endpoint},
  routes::Route,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    command_row::CommandRow,
    icon::Icon,
    popover::{PopoverSide, popover, popover_panel},
  },
};

/// The popover's width (`QqBCj`).
const POPOVER_WIDTH: f32 = 464.0;

#[derive(Clone)]
pub struct McpItemProps {
  pub status: McpStatus,
  pub dispatch: Dispatch,
}

/// Props are compared by what the item shows; the dispatch goes to the same window.
impl PartialEq for McpItemProps {
  fn eq(&self, other: &Self) -> bool {
    self.status == other.status
  }
}

impl DevtoolsInspectable for McpItemProps {}

/// The item, and its popover while it is open.
pub fn mcp_item(ctx: &mut Ctx, status: &McpStatus, dispatch: &Dispatch) -> Element {
  ctx.mount_keyed::<McpItem>(
    "status-mcp-item",
    McpItemProps {
      status: status.clone(),
      dispatch: dispatch.clone(),
    },
  )
}

pub struct McpItem {
  anchor: ElementRef,
  open: Signal<bool>,
}

impl Component for McpItem {
  type Props = McpItemProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      anchor: ctx.element_ref(),
      open: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<McpItemProps>().clone();
    let (words, color) = words(ctx, &props.status);
    let toggle = self.open.clone();
    let open = self.open.get();
    let mut item = pressed("status-mcp", move || toggle.update(|open| *open = !*open))
      .ref_element(self.anchor.clone())
      .describe("state", state_name(&props.status.server))
      .child(glyph(Icon::RadioTower, color.clone()))
      .child(label(&words, color).id("status-mcp-text"));
    if open {
      item = item.background(ExtraColor::Hover.role());
    }
    let mut row = Row::new().child(item);
    if open {
      let panel = panel(ctx, &props, &self.open);
      row = row.child(popover(
        self.anchor.clone(),
        self.open.clone(),
        PopoverSide::AboveEnd,
        panel,
      ));
    }
    row
  }
}

/// What the item says, and in which color.
fn words(ctx: &Ctx, status: &McpStatus) -> (String, PaletteColor) {
  match (&status.kind, &status.server) {
    (_, ServerState::PortTaken { port }) => (
      ctx
        .t_args("shell.status.mcpPortTaken", [("port", port.get().to_string())])
        .to_string(),
      PaletteColor::Danger,
    ),
    (_, ServerState::Off) => (ctx.t("shell.status.mcpOff").to_string(), PaletteColor::TextMuted),
    (_, ServerState::TokenUnreadable { .. } | ServerState::Unavailable) => {
      (ctx.t("shell.status.mcpUnavailable").to_string(), PaletteColor::Danger)
    }
    (McpKind::Installation, ServerState::Listening { port }) => (
      ctx
        .t_args("shell.status.mcpListening", [("address", address(*port).to_string())])
        .to_string(),
      PaletteColor::TextMuted,
    ),
    (McpKind::Qa { .. }, ServerState::Listening { port }) => (
      ctx
        .t_args("shell.status.mcpQa", [("port", port.to_string())])
        .to_string(),
      PaletteColor::TextMuted,
    ),
    (McpKind::Development { .. }, ServerState::Listening { port }) => (
      ctx
        .t_args("shell.status.mcpDevelopment", [("port", port.to_string())])
        .to_string(),
      PaletteColor::TextMuted,
    ),
  }
}

/// The state's name for tools.
pub const fn state_name(server: &ServerState) -> &'static str {
  match server {
    ServerState::Listening { .. } => "listening",
    ServerState::PortTaken { .. } => "port-taken",
    ServerState::Off => "off",
    ServerState::TokenUnreadable { .. } => "token-unreadable",
    ServerState::Unavailable => "unavailable",
  }
}

/// The popover's panel.
fn panel(ctx: &mut Ctx, props: &McpItemProps, open: &Signal<bool>) -> Element {
  let status = &props.status;
  let (dot_color, state) = state_line(ctx, status);
  let heading = Column::new()
    .spacing(ExtraSpacing::Fine.role())
    .child(Text::new(&ctx.t("shell.mcp.title")).variant(ExtraTypography::Strong.role()))
    .child(
      Row::new()
        .id("status-mcp-state")
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(dot(dot_color))
        .child(state),
    );
  let dispatch = props.dispatch.clone();
  let close = open.clone();
  let settings = ActionButton::new(
    "status-mcp-open-settings",
    ButtonKind::Ghost,
    ctx.t("shell.mcp.openSettings"),
  )
  .icon(Icon::Settings)
  .on_click(move || {
    close.set(false);
    dispatch(BarAction::Go(Route::Settings));
  });
  let listening = status.listening_port().is_some();
  popover_panel("status-mcp-popover", POPOVER_WIDTH)
    .child(heading)
    .child(
      Text::new(&body(ctx, status))
        .id("status-mcp-body")
        .variant(ExtraTypography::Prose.role())
        .width(Dimension::Pct(100.0)),
    )
    .with_children(where_to(ctx, status))
    .with_children(clients(ctx, status))
    .with_children(listening.then(|| window_control(ctx, status)))
    .child(settings)
    .into()
}

/// The dot and the words of the state line.
fn state_line(ctx: &Ctx, status: &McpStatus) -> (PaletteColor, Text) {
  match &status.server {
    ServerState::Listening { port } => (
      PaletteColor::Success,
      Text::new(&ctx.t_args("shell.mcp.listening", [("address", address(*port).to_string())]))
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextSecondary),
    ),
    ServerState::PortTaken { port } => (
      PaletteColor::Danger,
      Text::new(&ctx.t_args("shell.mcp.portTaken", [("port", port.get().to_string())]))
        .variant(ExtraTypography::Prose.role()),
    ),
    ServerState::Off => (
      PaletteColor::TextMuted,
      Text::new(&ctx.t("shell.mcp.off")).variant(ExtraTypography::Prose.role()),
    ),
    ServerState::TokenUnreadable { .. } | ServerState::Unavailable => (
      PaletteColor::Danger,
      Text::new(&ctx.t("shell.mcp.unavailable")).variant(ExtraTypography::Prose.role()),
    ),
  }
}

fn body(ctx: &Ctx, status: &McpStatus) -> String {
  let key = match (&status.kind, &status.server) {
    (_, ServerState::PortTaken { port }) => {
      return ctx
        .t_args("shell.mcp.body.portTaken", [("port", port.get().to_string())])
        .to_string();
    }
    (_, ServerState::Off) => "shell.mcp.body.off",
    (_, ServerState::TokenUnreadable { .. } | ServerState::Unavailable) => "shell.mcp.body.unavailable",
    (McpKind::Installation, ServerState::Listening { .. }) => token_store::LISTENING_BODY,
    (McpKind::Qa { .. }, ServerState::Listening { .. }) => "shell.mcp.body.qa",
    (McpKind::Development { .. }, ServerState::Listening { .. }) => "shell.mcp.body.development",
  };
  ctx.t(key).to_string()
}

/// Where clients find the server, with Copy: the address of the installed app, the discovery file of a copy.
fn where_to(ctx: &mut Ctx, status: &McpStatus) -> Option<Element> {
  let port = status.listening_port()?;
  let shown = match &status.kind {
    McpKind::Installation => endpoint(port),
    McpKind::Qa { folder } => folder.display().to_string(),
    McpKind::Development { file } => file.as_ref()?.display().to_string(),
  };
  let copied = shown.clone();
  Some(
    CommandRow::new("status-mcp-where", shown, ctx.t("common.copy"), move || {
      copy_to_clipboard(&copied)
    })
    .render(ctx),
  )
}

/// *2 agents connected: claude-code, codex-mcp-client*, as the clients connected now name themselves.
fn clients(ctx: &Ctx, status: &McpStatus) -> Option<Row> {
  let names: Vec<&str> = status
    .clients
    .iter()
    .filter(|client| client.connected)
    .map(|client| client.name.as_str())
    .collect();
  if names.is_empty() {
    return None;
  }
  let key = if names.len() == 1 {
    "shell.mcp.clients.one"
  } else {
    "shell.mcp.clients.other"
  };
  let text = ctx.t_args(key, [("count", names.len().to_string()), ("names", names.join(", "))]);
  Some(
    Row::new()
      .id("status-mcp-clients")
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(glyph(Icon::Plug, PaletteColor::TextMuted))
      .child(Text::new(&text).variant(ExtraTypography::Prose.role())),
  )
}

/// Whether agents may control the window, and what decided it, under a line.
fn window_control(ctx: &Ctx, status: &McpStatus) -> Column {
  let on = status.settings.window_control();
  let why = match status.settings.source() {
    WindowControlSource::Build => "shell.mcp.windowControl.build",
    WindowControlSource::Stored => "shell.mcp.windowControl.stored",
    WindowControlSource::Environment => "shell.mcp.windowControl.environment",
  };
  Column::new()
    .id("status-mcp-window-control")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Fine.role())
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(glyph(Icon::MousePointerClick, PaletteColor::TextMuted))
        .child(Text::new(&ctx.t("shell.mcp.windowControl.title")).variant(ExtraTypography::Prose.role()))
        .child(Spacer::new().flex(1.0))
        .child(
          Text::new(&ctx.t(if on {
            "shell.mcp.windowControl.on"
          } else {
            "shell.mcp.windowControl.off"
          }))
          .variant(ExtraTypography::Prose.role()),
        ),
    )
    .child(Text::new(&ctx.t(why)).variant(ExtraTypography::Note.role()))
}

/// Where the installed app keeps its token, as the popover names it.
mod token_store {
  #[cfg(target_os = "macos")]
  pub const LISTENING_BODY: &str = "shell.mcp.body.listeningMacos";
  #[cfg(not(target_os = "macos"))]
  pub const LISTENING_BODY: &str = "shell.mcp.body.listeningWindows";
}
