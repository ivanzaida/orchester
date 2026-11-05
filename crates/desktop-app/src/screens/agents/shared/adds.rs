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
use orchester_store::BoundReference;
use orchester_workspace::sharing::sync::{HereReference, Standing};

use super::Waiting;
use crate::{
  theme::{ExtraSpacing, HubText},
  ui::icon::{Icon, icon},
};

/// What a version adds, one line each with a plus (`uOQNr` in 02d): first each MCP server and secret it names by
/// name with how it stands here, *MCP server github · bound here to “GitHub (remote)”*, then the rest.
pub(super) fn adds_list(ctx: &Ctx, waiting: &Waiting) -> Column {
  let mut named: Vec<String> = Vec::new();
  let mut rest: Vec<String> = Vec::new();
  for add in &waiting.adds {
    let of_kind = |reference: &&HereReference| match add {
      VersionWidening::McpServer => matches!(reference.binding.reference, BoundReference::McpServer { .. }),
      VersionWidening::Secret => matches!(reference.binding.reference, BoundReference::Secret { .. }),
      _ => false,
    };
    let references: Vec<&HereReference> = waiting.added.iter().filter(of_kind).collect();
    if references.is_empty() {
      rest.push(ctx.t(adds_key(add)).to_string());
    } else {
      named.extend(references.into_iter().map(|reference| named_add(ctx, reference)));
    }
  }
  named.dedup();
  Column::new()
    .id("agent-widens-adds")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(ExtraSpacing::Snug.role()))
    .spacing(SpacingSize::Xs)
    .with_children(named.into_iter().chain(rest).enumerate().map(|(index, words)| {
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
    }))
}

/// An MCP server or secret a version adds, by name, with how it stands on this computer.
fn named_add(ctx: &Ctx, reference: &HereReference) -> String {
  let key = match reference.binding.reference {
    BoundReference::Secret { .. } => "sharing.config.widens.adds.secretNamed",
    _ => "sharing.config.widens.adds.serverNamed",
  };
  let what = ctx.t_args(key, [("name", reference.binding.name.as_str())]).to_string();
  let (key, local) = match &reference.standing {
    Standing::Bound { local, .. } => ("sharing.config.widens.adds.boundHere", local.clone()),
    Standing::Found { name, .. } => ("sharing.config.widens.adds.foundHere", name.clone()),
    Standing::Missing => ("sharing.config.widens.adds.missingHere", String::new()),
  };
  ctx.t_args(key, [("what", what), ("local", local)]).to_string()
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
    },
    VersionWidening::McpServer => "sharing.config.widens.adds.mcpServer",
    VersionWidening::Secret => "sharing.config.widens.adds.secret",
    VersionWidening::Command(_) => "sharing.config.widens.adds.command",
    VersionWidening::Address => "sharing.config.widens.adds.address",
    VersionWidening::Method => "sharing.config.widens.adds.method",
    VersionWidening::Headers => "sharing.config.widens.adds.headers",
    VersionWidening::Database => "sharing.config.widens.adds.database",
    VersionWidening::Query => "sharing.config.widens.adds.query",
    VersionWidening::Access => "sharing.config.widens.adds.access",
  }
}
