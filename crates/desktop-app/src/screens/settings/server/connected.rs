//! *Agents connected* (`Agents connected` `PBBUM` and its list `J8KUDH` in `04 · Automation`): the clients that
//! connected since the app started, or since the token changed, listed under the row as each named itself, with the
//! maker's mark of a CLI Orchester knows by that name, its version, and *connected now* or when it last asked
//! something. The list is the server's, in memory: nothing is stored, and no time it connected is known.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::display;

use crate::{
  mcp::{McpStatus, SeenClient},
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    agent_mark::{AgentMark, agent_mark},
    icon::{Icon, icon},
    setting_rows::{SettingRow, status},
  },
};

pub fn connected(ctx: &mut Ctx, status: &McpStatus) -> Element {
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
      .variant(ExtraTypography::Note.role())
      .into()
  } else {
    let rows: Vec<Element> = status
      .clients
      .iter()
      .enumerate()
      .map(|(index, client)| client_row(ctx, index, client))
      .collect();
    Column::new()
      .id("settings-mcp-connected-list")
      .width(Dimension::Pct(100.0))
      .with_children(rows)
      .into()
  };
  SettingRow::new("settings-mcp-connected", ctx.t("settings.automation.connected.title"))
    .hint(ctx.t(description))
    .below(list)
    .render(ctx)
}

/// *✳ claude-code 2.1.296 · ● connected now* (`Claude Code` `KC73f`).
fn client_row(ctx: &Ctx, index: usize, client: &SeenClient) -> Element {
  let (colors, when) = if client.connected {
    (
      (PaletteColor::Success, PaletteColor::Success),
      ctx.t("settings.automation.connected.now"),
    )
  } else {
    (
      (PaletteColor::TextMuted, PaletteColor::TextMuted),
      ctx.t_args(
        "settings.automation.connected.lastSeen",
        [("time", display::clock_time(client.last_request))],
      ),
    )
  };
  let mark = AgentMark::of_client(&client.name).map_or_else(
    || icon(Icon::Plug, IconSize::Md, PaletteColor::TextMuted).into(),
    |mark| agent_mark(ctx, mark, IconSize::Md.size()),
  );
  Row::new()
    .id(format!("settings-mcp-client-{index}"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(ExtraSpacing::Snug.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(mark)
    .child(
      Text::new(&client.name)
        .id(format!("settings-mcp-client-{index}-name"))
        .variant(ExtraTypography::OptionTitle.role())
        .nowrap(),
    )
    .child(
      Text::new(&client.version)
        .id(format!("settings-mcp-client-{index}-version"))
        .variant(ExtraTypography::Note.role())
        .nowrap(),
    )
    .child(Spacer::new().flex(1.0))
    .child(status(&format!("settings-mcp-client-{index}-state"), colors, &when))
    .into()
}
