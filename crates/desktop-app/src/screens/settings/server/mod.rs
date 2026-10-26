//! *Orchester MCP server* in Settings › Automation (`Orchester MCP server` `kgrfZ` in `04 · Automation` of the new
//! `orchester.pen`; its states on the sheet `shpsk`): for the installed app, whether agents may connect, the fixed
//! address with Copy and the port field it opens, the access token with Show, Copy and Rotate, the commands that
//! register Orchester in Claude Code and Codex, and the agents connected since the app started (kept in memory only).
//! A QA copy or a development build says instead that it takes a new port on each start and where its discovery file
//! is, and its group's heading names which it is.
//!
//! Every control here is a person's: while the group is mounted the app's MCP server refuses window input
//! (`Guarded::McpServer`), and a change synthetic input brings is not taken and the row says so. The changes run off
//! the UI thread; what came of them arrives in the server's status.

mod address;
mod connect;
mod connected;
mod enabled;
mod token;

use std::num::NonZeroU16;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, TypographyStyle},
  },
  components::Text,
  node::Element,
};
use orchester_core::app_mcp::McpPort;
use orchester_workspace::{background, messages::Message};

use crate::{
  mcp::{
    AppMcp, ControlError, McpKind, McpLink, McpStatus, ServerState, address as socket,
    input_gate::{Guarded, InputGate, Protected},
  },
  shell::status_bar::{from_scratch, mcp_status},
  ui::{
    icon::Icon,
    notice::NoticeTone,
    setting_notice::{SettingNotice, status_line},
    setting_rows::{SettingGroup, SettingRow, status},
  },
};

/// The group; it reads the window's server from the context.
pub struct ServerSection {
  /// While the group is mounted, the MCP server refuses window input.
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
      McpKind::Qa { folder, .. } => {
        // From the worktree's `.tmp` on, as the status bar names it.
        let folder = from_scratch(folder);
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

/// The group's heading, with which build it is at its end.
fn group(ctx: &Ctx, meta: &str) -> SettingGroup {
  SettingGroup::new("settings-mcp", ctx.t("settings.automation.server.section")).end(
    Text::new(&ctx.t(meta))
      .id("settings-mcp-meta")
      .variant(TypographyStyle::Description)
      .color(PaletteColor::TextMuted)
      .nowrap(),
  )
}

/// The installed app's rows.
fn installation(ctx: &mut Ctx, status: &McpStatus, server: Option<AppMcp>) -> Element {
  let port = status.port.and_then(|port| NonZeroU16::new(port.get()));
  // The commands carry the token: while it cannot be read, Connect an agent says so instead.
  let commands = port.filter(|_| !matches!(status.server, ServerState::TokenUnreadable { .. }));
  let enabled = ctx.mount_keyed::<enabled::EnabledRow>(
    "settings-mcp-enabled",
    enabled::Props {
      server: server.clone(),
      state: status.server.clone(),
    },
  );
  let address = ctx.mount_keyed::<address::AddressRow>(
    "settings-mcp-address",
    address::Props {
      server: server.clone(),
      state: status.server.clone(),
      port: status.port,
    },
  );
  let token = ctx.mount_keyed::<token::TokenRow>(
    "settings-mcp-token",
    token::Props {
      server: server.clone(),
      state: status.server.clone(),
    },
  );
  let connect = ctx.mount_keyed::<connect::ConnectBlock>("settings-mcp-connect", connect::Props { server, port: commands });
  let connected = connected::connected(ctx, status);
  group(ctx, "settings.automation.server.meta.installation")
    .rows([enabled, address, token, connect, connected])
    .into()
}

/// A QA copy's or a development build's group: where it listens this time, and where its discovery file is.
fn copy_section(ctx: &mut Ctx, status: &McpStatus, (meta, says): (&str, &str), file: &str) -> Element {
  let state = match &status.server {
    ServerState::Listening { port } => listening_on(ctx, *port),
    _ => state_label(ctx, &status.server),
  };
  let server = SettingRow::new("settings-mcp-server", ctx.t("settings.automation.server.copy.title"))
    .hint(ctx.t(says))
    .control(state)
    .below(status_line("settings-mcp-discovery", Icon::FlaskConical, file))
    .render(ctx);
  group(ctx, meta).row(server).into()
}

/// *● Listening on 127.0.0.1:53122*.
fn listening_on(ctx: &Ctx, port: NonZeroU16) -> Element {
  let words = ctx.t_args(
    "settings.automation.server.copy.listening",
    [("address", socket(port).to_string())],
  );
  status(
    "settings-mcp-state",
    (PaletteColor::Success, PaletteColor::Success),
    &words,
  )
  .into()
}

/// The server's state: *● Listening*, *● Port 47821 taken*, *● Off*, *● Not running*.
fn state_label(ctx: &Ctx, state: &ServerState) -> Element {
  let (dot, color, words) = match state {
    ServerState::Listening { .. } => (
      PaletteColor::Success,
      PaletteColor::Success,
      ctx.t("settings.automation.server.state.listening"),
    ),
    ServerState::PortTaken { port, how } => (
      PaletteColor::Danger,
      PaletteColor::Danger,
      ctx.t_args(
        how.pick(
          "settings.automation.server.state.portTaken",
          "settings.automation.server.state.portRefused",
        ),
        [("port", port.get().to_string())],
      ),
    ),
    ServerState::Off => (
      PaletteColor::TextMuted,
      PaletteColor::TextMuted,
      ctx.t("settings.automation.server.state.off"),
    ),
    ServerState::TokenUnreadable { .. } | ServerState::Unavailable => (
      PaletteColor::Danger,
      PaletteColor::Danger,
      ctx.t("settings.automation.server.state.notRunning"),
    ),
  };
  status("settings-mcp-state", (dot, color), &words).into()
}

/// What came of a change that went through, as a success notice (`Notice · success` `grux3` in `shpsk`).
fn outcome(id: &str, text: &str) -> Element {
  SettingNotice::new(id, NoticeTone::Success).body(text).into()
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
