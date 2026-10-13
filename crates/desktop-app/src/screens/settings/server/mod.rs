//! *Orchester MCP server* in Settings › Automation (`lye6f`; its states `eTrCO`, the token `eM8tB`, *Connect an
//! agent* `NzOSY`): for the installed app, whether agents may connect, the fixed address and its port, the access
//! token with Show, Copy and Rotate, the commands that register Orchester in Claude Code and Codex, and the agents
//! connected since the app started (kept in memory only). A QA copy or a development build says instead that it
//! takes a new port on each start and where its discovery file is.
//!
//! Every control here is a person's: while the section is mounted the app's MCP server refuses window input
//! (`Guarded::McpServer`), and a change synthetic input brings is not taken and the row says so. The changes run
//! off the UI thread; what came of them arrives in the server's status.

mod address;
mod connect;
mod connected;
mod copy;
mod enabled;
mod token;

use std::num::NonZeroU16;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::PaletteColor,
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::app_mcp::McpPort;
use orchester_workspace::{background, messages::Message};

use crate::{
  mcp::{
    AppMcp, ControlError, McpKind, McpLink, McpStatus, ServerState, address as socket,
    input_gate::{Guarded, InputGate, Protected},
  },
  shell::status_bar::mcp_status,
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    form_section::{Label, Place, section, side_row},
    icon::{Icon, icon},
    inspector::note,
  },
};

/// The section; it reads the window's server from the context.
pub struct ServerSection {
  /// While the section is mounted, the MCP server refuses window input.
  _protected: Protected,
}

impl Component for ServerSection {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    let gate = ctx
      .use_context::<InputGate>()
      .expect("the session's window provides the MCP input gate");
    Self {
      _protected: gate.protect(Guarded::McpServer),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let link = ctx
      .use_context::<McpLink>()
      .expect("the root provides the MCP server's link");
    let status = mcp_status(ctx, &link);
    let server = link.server().cloned();
    match &status.kind {
      McpKind::Installation => installation(ctx, &status, server),
      McpKind::Qa { folder } => {
        let folder = folder.display().to_string();
        let note = ctx.t_args("settings.automation.server.copy.qaNote", [("folder", folder)]);
        let keys = (
          "settings.automation.server.meta.qa",
          "settings.automation.server.copy.qa",
        );
        copy_section(ctx, &status, keys, &note)
      }
      McpKind::Development { file } => {
        let note = file.as_ref().map_or_else(
          || ctx.t("settings.automation.server.copy.developmentNoFile"),
          |file| {
            ctx.t_args(
              "settings.automation.server.copy.developmentNote",
              [("file", file.display().to_string())],
            )
          },
        );
        let keys = (
          "settings.automation.server.meta.development",
          "settings.automation.server.copy.development",
        );
        copy_section(ctx, &status, keys, &note)
      }
    }
  }
}

/// The installed app's rows.
fn installation(ctx: &mut Ctx, status: &McpStatus, server: Option<AppMcp>) -> Element {
  let port = status.port.and_then(|port| NonZeroU16::new(port.get()));
  // The commands carry the token: while it cannot be read, Connect an agent says so instead.
  let commands = port.filter(|_| !matches!(status.server, ServerState::TokenUnreadable { .. }));
  let mut rows = vec![ctx.mount_keyed::<enabled::EnabledRow>(
    "settings-mcp-enabled",
    enabled::Props {
      server: server.clone(),
      state: status.server.clone(),
    },
  )];
  rows.push(ctx.mount_keyed::<address::AddressRow>(
    "settings-mcp-address",
    address::Props {
      server: server.clone(),
      state: status.server.clone(),
      port: status.port,
    },
  ));
  rows.push(ctx.mount_keyed::<token::TokenRow>(
    "settings-mcp-token",
    token::Props {
      server: server.clone(),
      state: status.server.clone(),
    },
  ));
  rows
    .push(ctx.mount_keyed::<connect::ConnectBlock>("settings-mcp-connect", connect::Props { server, port: commands }));
  rows.push(connected::connected(ctx, status));
  section(
    "settings-mcp",
    &ctx.t("settings.automation.server.section"),
    Some(&ctx.t("settings.automation.server.meta.installation")),
    rows,
  )
  .into()
}

/// A QA copy's or a development build's section: where it listens this time, and where its discovery file is.
fn copy_section(ctx: &mut Ctx, status: &McpStatus, (meta, says): (&str, &str), file: &str) -> Element {
  let state = match &status.server {
    ServerState::Listening { port } => listening_on(ctx, *port),
    _ => state_label(ctx, &status.server),
  };
  let server = side_row(
    Label::new("settings-mcp-server", ctx.t("settings.automation.server.copy.title")).description(ctx.t(says)),
    state,
    Place::Within,
    false,
  );
  section(
    "settings-mcp",
    &ctx.t("settings.automation.server.section"),
    Some(&ctx.t(meta)),
    vec![
      server,
      Row::new()
        .width(Dimension::Pct(100.0))
        .padding_vertical(ExtraSpacing::Loose.role())
        .child(note("settings-mcp-discovery", Icon::FlaskConical, file))
        .into(),
    ],
  )
  .into()
}

/// *● Listening on 127.0.0.1:53122*.
fn listening_on(ctx: &Ctx, port: NonZeroU16) -> Element {
  let words = ctx.t_args(
    "settings.automation.server.copy.listening",
    [("address", socket(port).to_string())],
  );
  dotted(
    "settings-mcp-state",
    PaletteColor::Success,
    Text::new(&words).variant(ExtraTypography::MonoKey.role()),
  )
}

/// The server's state beside the switch: *● Listening*, *● Port 47821 taken*, *● Off*, *● Not running*.
fn state_label(ctx: &Ctx, state: &ServerState) -> Element {
  let (color, words) = match state {
    ServerState::Listening { .. } => (
      PaletteColor::Success,
      ctx.t("settings.automation.server.state.listening"),
    ),
    ServerState::PortTaken { port } => (
      PaletteColor::Danger,
      ctx.t_args(
        "settings.automation.server.state.portTaken",
        [("port", port.get().to_string())],
      ),
    ),
    ServerState::Off => (PaletteColor::TextMuted, ctx.t("settings.automation.server.state.off")),
    ServerState::TokenUnreadable { .. } | ServerState::Unavailable => (
      PaletteColor::Danger,
      ctx.t("settings.automation.server.state.notRunning"),
    ),
  };
  dotted(
    "settings-mcp-state",
    color,
    Text::new(&words).variant(ExtraTypography::Prose.role()),
  )
}

/// `words` after a 6px dot in `color`.
fn dotted(id: &str, color: PaletteColor, words: Text) -> Element {
  Row::new()
    .id(id.to_owned())
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(crate::shell::status_bar::dot(color))
    .child(words.nowrap())
    .into()
}

/// A line that says what came of a change: its icon and words in `color` (`text_muted` for a note).
fn outcome(id: &str, glyph: Icon, color: PaletteColor, text: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Start)
    .child(icon(glyph, IconSize::Sm, color.clone()).flex_shrink(0.0))
    .child(
      Text::new(text)
        .id(format!("{id}-text"))
        .variant(ExtraTypography::Note.role())
        .color(color)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}

/// A change of the server, run off the UI thread; what came of it shows in the server's status, a failure here.
type Change<A> = FutureAction<A, (), Message>;

fn change<A: Clone + Send + Sync + 'static>(
  ctx: &mut Ctx,
  server: Option<AppMcp>,
  run: fn(&AppMcp, A) -> Result<(), ControlError>,
) -> Change<A> {
  ctx.future_action(move |argument: A| {
    let server = server.clone();
    async move {
      let Some(server) = server else {
        return Err(ControlError::NotInstallation.message());
      };
      background::run(move || run(&server, argument)).await?.map_err(|error| {
        tracing::error!(%error, "the app's MCP server was not changed");
        error.message()
      })
    }
  })
}

/// The port as a [`McpPort`]'s digits, for the field.
fn port_text(port: Option<McpPort>) -> String {
  port.map(|port| port.get().to_string()).unwrap_or_default()
}
