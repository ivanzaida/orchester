//! A tool in the library (`sFtAz`, as `Sgrkv` in `ZFwWl`): icon, name and what it does, its type,
//! its secrets, its status and a chevron. Selecting a row opens the tool in its editor.

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
use orchester_core::resource::{ToolAction, ToolKind};

use crate::{
  agents::data::VersionState,
  theme::{ExtraColor, ExtraRadius},
  tools::data::ToolEntry,
  ui::{
    icon::{Icon, IconSize, icon},
    pressable::{pressable, surface},
    status_label::compact_status_label,
  },
};

/// Icon surface and the type and secrets columns (`lhuOK`, `JIsjZ`, `f6poW` in `sFtAz`), and the
/// row height in the library (`Sgrkv`).
const ICON_SURFACE: f32 = 36.0;
const TYPE_WIDTH: f32 = 100.0;
const SECRETS_WIDTH: f32 = 78.0;
const ROW_HEIGHT: f32 = 76.0;

/// The icon, type and secrets copy of a tool kind.
pub(super) const fn kind_icon(kind: ToolKind) -> Icon {
  match kind {
    ToolKind::Command => Icon::Terminal,
    ToolKind::Http => Icon::Globe,
    ToolKind::Database => Icon::Database,
  }
}

pub(super) const fn kind_key(kind: ToolKind) -> &'static str {
  match kind {
    ToolKind::Command => "tools.kind.command",
    ToolKind::Http => "tools.kind.http",
    ToolKind::Database => "tools.kind.database",
  }
}

/// Status of a tool: ready once published, a draft before, and unavailable for kinds that cannot
/// run yet.
pub(super) fn status(ctx: &Ctx, entry: &ToolEntry) -> (Icon, Arc<str>) {
  if !entry.tool.kind().runnable() {
    return (Icon::Lock, ctx.t("tools.status.unavailable"));
  }
  match entry.state.published() {
    Some(_) => (Icon::CircleCheck, ctx.t("tools.status.ready")),
    None => (Icon::Pencil, ctx.t("tools.status.draft")),
  }
}

pub(super) fn tool_row(ctx: &Ctx, entry: &ToolEntry, open: impl Fn() + Send + Sync + 'static) -> Element {
  let kind = entry.tool.kind();
  let description: Arc<str> = match (entry.tool.description(), entry.tool.action()) {
    ("", ToolAction::Command(command)) => command.command_line().into(),
    ("", _) => ctx.t("tools.noDescription"),
    (description, _) => description.into(),
  };
  let secrets = if kind == ToolKind::Command {
    ctx.t("tools.secrets.none")
  } else {
    ctx.t("tools.secrets.notAvailable")
  };
  let identity = Column::new()
    .flex(1.0)
    .spacing(SpacingSize::Sm)
    .child(Text::new(&entry.name).variant(TypographyStyle::Label).nowrap())
    .child(
      Text::new(&description)
        .variant(TypographyStyle::Caption)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    );
  let (glyph, state) = status(ctx, entry);
  let row = surface()
    .id(Arc::<str>::from(format!("tool-{}", entry.id)))
    .describe("tool", entry.name.to_string())
    .describe("draft", matches!(entry.state, VersionState::Draft { .. }).to_string())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(icon_surface(kind_icon(kind)))
    .child(identity)
    .child(caption(&ctx.t(kind_key(kind)), TYPE_WIDTH))
    .child(caption(&secrets, SECRETS_WIDTH))
    .child(compact_status_label(glyph, &state))
    .child(icon(Icon::ChevronRight, IconSize::Dismiss, PaletteColor::TextSecondary));
  // Rows are outside a form, so they are pointer-only: no Tab stop and no focus ring.
  pressable(row, open)
    .hovered_style(Style::new().background(PaletteColor::SurfaceRaised))
    .into()
}

fn icon_surface(glyph: Icon) -> Element {
  Row::new()
    .size(Dimension::Px(ICON_SURFACE), Dimension::Px(ICON_SURFACE))
    .background(PaletteColor::SurfaceRaised)
    .rounded(ExtraRadius::Notice.role())
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(icon(glyph, IconSize::Notice, PaletteColor::TextPrimary))
    .into()
}

fn caption(text: &str, width: f32) -> Element {
  Column::new()
    .width(Dimension::Px(width))
    .child(Text::new(text).variant(TypographyStyle::Caption).nowrap())
    .into()
}
