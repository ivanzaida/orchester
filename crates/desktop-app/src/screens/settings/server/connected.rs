//! *Agents connected* (`lye6f`, `eM8tB`): the clients that connected since the app started, or since the token
//! changed, as each named itself, with *connected now* or when it last asked something. The list is the server's,
//! in memory: nothing is stored.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::display;

use super::dotted;
use crate::{
  mcp::{McpStatus, SeenClient},
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    form_section::{Label, Place, stacked_row},
    icon::{Icon, icon},
  },
};

pub fn connected(ctx: &Ctx, status: &McpStatus) -> Element {
  let (description, none) = if status.rotated {
    (
      "settings.automation.connected.descriptionRotated",
      "settings.automation.connected.noneRotated",
    )
  } else {
    (
      "settings.automation.connected.description",
      "settings.automation.connected.none",
    )
  };
  let list: Element = if status.clients.is_empty() {
    Text::new(&ctx.t(none))
      .id("settings-mcp-connected-none")
      .variant(ExtraTypography::Prose.role())
      .into()
  } else {
    Column::new()
      .id("settings-mcp-connected-list")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role())
      .with_children(
        status
          .clients
          .iter()
          .enumerate()
          .map(|(index, client)| client_row(ctx, index, client)),
      )
      .into()
  };
  stacked_row(
    Label::new("settings-mcp-connected", ctx.t("settings.automation.connected.title")).description(ctx.t(description)),
    list,
    Place::Last,
  )
}

/// *claude-code 2.1.296 · ● connected now*.
fn client_row(ctx: &Ctx, index: usize, client: &SeenClient) -> Element {
  let name = ctx.t_args(
    "settings.automation.connected.client",
    [("name", client.name.as_str()), ("version", client.version.as_str())],
  );
  let (color, when) = if client.connected {
    (PaletteColor::Success, ctx.t("settings.automation.connected.now"))
  } else {
    (
      PaletteColor::TextMuted,
      ctx.t_args(
        "settings.automation.connected.lastSeen",
        [("time", display::clock_time(client.last_request))],
      ),
    )
  };
  Row::new()
    .id(format!("settings-mcp-client-{index}"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Plug, IconSize::Sm, PaletteColor::TextMuted))
    .child(Text::new(&name).variant(ExtraTypography::PropertyMono.role()).nowrap())
    .child(Spacer::new().flex(1.0))
    .child(dotted(
      &format!("settings-mcp-client-{index}-state"),
      color,
      Text::new(&when).variant(ExtraTypography::Prose.role()),
    ))
    .into()
}
