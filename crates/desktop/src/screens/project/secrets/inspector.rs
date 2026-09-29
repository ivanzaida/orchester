//! The shown secret's details (`CQxCb`): its kind, name and masked value, where it belongs, when
//! its value was last replaced, and the tools or MCP servers that use it, read-only. Replace value
//! and Archive secret stay at the bottom; the usage log
//! does not exist yet.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::{
    Alignment,
    layout_kind::{Justify, ScrollState},
  },
  node::{CursorIcon, Element, border::Border, dimension::Dimension},
};
use orchester_core::secret::Secret;

use super::{
  ProjectNav,
  labels::{date, environment_key, kind_icon, kind_key},
};
use crate::{
  navigation::route::{ConnectionTab, Page},
  screens::project::page::TOPBAR_BUTTON_HEIGHT,
  secrets::data::{SecretUse, SecretUseKind, UseCounts},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, gutter_scrollbar},
  ui::{
    button::ActionButton,
    icon::{Icon, IconSize, icon},
    pressable::{pressable, surface},
    property::property,
  },
};

/// Inspector width (`CQxCb`), the height of the stored value box (`D4Jja`) and of a tool link
/// (`iYp1a`).
const INSPECTOR_WIDTH: f32 = 328.0;
const STORED_VALUE_HEIGHT: f32 = 41.0;
const TOOL_LINK_HEIGHT: f32 = 40.0;

type Handler = Arc<dyn Fn() + Send + Sync>;

pub struct InspectorActions {
  pub replace: Handler,
  pub archive: Handler,
}

pub fn inspector(
  ctx: &Ctx,
  (secret, scroll): (&Secret, &ScrollState),
  project: &str,
  (uses, counts): (&[&SecretUse], UseCounts),
  nav: &ProjectNav,
  actions: &InspectorActions,
) -> Element {
  let details = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Step.role())
    .child(icon(
      kind_icon(secret.kind()),
      IconSize::Large,
      PaletteColor::TextPrimary,
    ))
    .child(
      Text::new(&ctx.t(kind_key(secret.kind())))
        .variant(ExtraTypography::InspectorTitle.role())
        .id("secret-inspector-title"),
    )
    .child(
      Text::new(secret.name().as_str())
        .variant(TypographyStyle::Mono)
        .id("secret-inspector-name"),
    )
    .child(stored_value(ctx))
    .child(properties(ctx, secret, project))
    .child(used_by(ctx, uses, counts, project, nav));
  let replace = actions.replace.clone();
  let pinned = Column::new()
    .width(Dimension::Pct(100.0))
    .padding_right(ExtraSpacing::Stack.role())
    .spacing(ExtraSpacing::Step.role())
    .child(
      ActionButton::new(ctx.t("secrets.replaceValue"))
        .icon(Icon::RefreshCw)
        .id("replace-secret-value")
        .fill_width()
        .height(Dimension::Px(TOPBAR_BUTTON_HEIGHT))
        .on_click(move || replace())
        .render(ctx),
    )
    .child(lifecycle_actions(ctx, &actions.archive));
  // The details scroll when they do not fit, their bar in a gutter of its own; the actions stay
  // at the bottom, as in the connection inspectors.
  Column::new()
    .id("secret-inspector")
    .width(Dimension::Px(INSPECTOR_WIDTH))
    .height(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Lg)
    .padding_left(SpacingSize::Lg)
    .padding_right(ExtraSpacing::Stack.role())
    .spacing(ExtraSpacing::Step.role())
    .background(PaletteColor::SurfacePanel)
    .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      ScrollVertical::new(details)
        .id("secret-inspector-details")
        .with_scroll_state(scroll.clone())
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .scrollbar(gutter_scrollbar()),
    )
    .child(pinned)
    .into()
}

/// The value as a mask with a lock (`D4Jja`). It is never read back: the keychain keeps it.
fn stored_value(ctx: &Ctx) -> Element {
  Row::new()
    .id("secret-stored-value")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(STORED_VALUE_HEIGHT))
    .padding(ExtraSpacing::Stack.role())
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .background(ExtraColor::Surface.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(
      Text::new(&ctx.t("secrets.inspector.masked"))
        .variant(ExtraTypography::MaskedValue.role())
        .flex(1.0),
    )
    .child(icon(Icon::LockKeyhole, IconSize::Dismiss, PaletteColor::TextSecondary))
    .into()
}

/// Project, environment and last rotation (`I4GsS`).
fn properties(ctx: &Ctx, secret: &Secret, project: &str) -> Element {
  let rotated = secret.rotated_at().map_or_else(
    || {
      ctx.t_args(
        "secrets.inspector.neverRotated",
        [("date", &*date(ctx, secret.created_at()))],
      )
    },
    |at| date(ctx, at),
  );
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(property(&ctx.t("secrets.property.project"), project))
    .child(property(
      &ctx.t("secrets.property.environment"),
      &ctx.t(environment_key(secret.environment())),
    ))
    .child(property(&ctx.t("secrets.property.lastRotated"), &rotated))
    .into()
}

/// The tools and MCP servers that use the secret (`ABEPt`). The Tools screen does not exist yet.
fn used_by(ctx: &Ctx, uses: &[&SecretUse], counts: UseCounts, project: &str, nav: &ProjectNav) -> Element {
  let heading = match (counts.tools, counts.mcp_servers) {
    (0, 0) => ctx.t("secrets.inspector.usedBy.none"),
    (1, 0) => ctx.t("secrets.inspector.usedBy.one"),
    (count, 0) => ctx.t_args(
      "secrets.inspector.usedBy.other",
      [("count", count.to_string().as_str())],
    ),
    (0, 1) => ctx.t("secrets.inspector.usedBy.mcpOne"),
    (0, count) => ctx.t_args(
      "secrets.inspector.usedBy.mcpOther",
      [("count", count.to_string().as_str())],
    ),
    _ => ctx.t_args(
      "secrets.inspector.usedBy.mixed",
      [("count", counts.total().to_string().as_str())],
    ),
  };
  let links: Vec<Element> = if uses.is_empty() {
    vec![
      Text::new(&ctx.t("secrets.inspector.notUsed"))
        .variant(TypographyStyle::Caption)
        .into(),
    ]
  } else {
    uses
      .iter()
      .map(|used| {
        let label = match used.kind {
          SecretUseKind::Tool(_) => used.name.clone(),
          SecretUseKind::McpServer(_) => ctx.t_args("secrets.inspector.mcpLink", [("name", &*used.name)]),
        };
        let button = ActionButton::new(label)
          .icon(Icon::ExternalLink)
          .fill_width()
          .height(Dimension::Px(TOOL_LINK_HEIGHT));
        match used.kind {
          SecretUseKind::Tool(_) => button.render(ctx),
          SecretUseKind::McpServer(id) => button
            .link(nav.link(Page::Connections(ConnectionTab::McpServers, Some(id))))
            .render(ctx),
        }
      })
      .collect()
  };
  let note_key = match (counts.tools > 0, counts.mcp_servers > 0) {
    (false, true) => "secrets.inspector.mcpNote",
    (true, true) => "secrets.inspector.mixedNote",
    _ => "secrets.inspector.toolsNote",
  };
  Column::new()
    .id("secret-used-by")
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Step.role())
    .spacing(ExtraSpacing::Stack.role())
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(&heading).variant(ExtraTypography::Overline.role()))
    .with_children(links)
    .child(Text::new(&ctx.t_args(note_key, [("project", project)])).variant(ExtraTypography::Detail.role()))
    .into()
}

/// View usage log, which does not exist yet, and Archive secret (`D7vhEl`).
fn lifecycle_actions(ctx: &Ctx, archive: &Handler) -> Element {
  let usage_log = Row::new()
    .id("secret-usage-log")
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .describe("state", "unavailable")
    .cursor(CursorIcon::NotAllowed)
    .child(Text::new(&ctx.t("secrets.inspector.usageLog")).variant(TypographyStyle::Caption))
    .child(icon(Icon::Lock, IconSize::Small, PaletteColor::TextMuted));
  let archive = archive.clone();
  let archive_link = pressable(surface().id("archive-secret"), move || archive())
    .child(Text::new(&ctx.t("secrets.inspector.archive")).variant(TypographyStyle::Caption));
  Row::new()
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .justify(Justify::SpaceBetween)
    .child(usage_log)
    .child(archive_link)
    .into()
}
