//! What a teammate's version adds, as the widens bar lists it (`uOQNr` in 02d `FuOXA`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{dimension::Dimension, padding::Padding},
};
use orchester_core::{permission::widening::Widening, sharing::VersionWidening};

use super::Waiting;
use crate::{
  theme::{ExtraSpacing, HubText},
  ui::icon::{Icon, icon},
};

/// What a version adds, one line each with a plus (`uOQNr` in 02d).
pub(super) fn adds_list(ctx: &Ctx, waiting: &Waiting) -> Column {
  Column::new()
    .id("agent-widens-adds")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(ExtraSpacing::Snug.role()))
    .spacing(SpacingSize::Xs)
    .with_children(
      waiting
        .adds
        .iter()
        .map(|add| ctx.t(adds_key(add)))
        .enumerate()
        .map(|(index, words)| {
          Row::new()
            .id(format!("agent-widens-add-{index}"))
            .width(Dimension::Pct(100.0))
            .spacing(SpacingSize::Sm)
            .align_items(Alignment::Center)
            .child(icon(
              Icon::Plus,
              crate::theme::IconSize::Sm,
              PaletteColor::TextSecondary,
            ))
            .child(
              Text::new(&words)
                .variant(HubText::Status.role())
                .flex(1.0)
                .min_width(Dimension::Px(0.0)),
            )
        }),
    )
}

/// The few words that say what a version adds (`Add` rows in `uOQNr`).
fn adds_key(add: &VersionWidening) -> &'static str {
  match add {
    VersionWidening::Permissions(widening) => match widening {
      Widening::Repository => "sharing.config.widens.adds.repository",
      Widening::WritableFolder => "sharing.config.widens.adds.writableFolder",
      Widening::Commands => "sharing.config.widens.adds.commands",
      Widening::Network => "sharing.config.widens.adds.network",
      Widening::ToolCallsUnasked => "sharing.config.widens.adds.toolCallsUnasked",
      Widening::Autopilot => "sharing.config.widens.adds.autopilot",
      Widening::Browser => "sharing.config.widens.adds.browser",
      Widening::BrowserSite => "sharing.config.widens.adds.browserSite",
      Widening::CodexSandboxOff => "sharing.config.widens.adds.codexSandboxOff",
    },
    VersionWidening::McpServer => "sharing.config.widens.adds.mcpServer",
    VersionWidening::Secret => "sharing.config.widens.adds.secret",
    VersionWidening::Tool => "sharing.config.widens.adds.tool",
    VersionWidening::Command(_) => "sharing.config.widens.adds.command",
    VersionWidening::Address => "sharing.config.widens.adds.address",
    VersionWidening::Method => "sharing.config.widens.adds.method",
    VersionWidening::Headers => "sharing.config.widens.adds.headers",
    VersionWidening::Database => "sharing.config.widens.adds.database",
    VersionWidening::Query => "sharing.config.widens.adds.query",
    VersionWidening::Access => "sharing.config.widens.adds.access",
  }
}
