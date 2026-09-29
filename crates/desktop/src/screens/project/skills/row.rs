//! A skill in the library (`hn8qO` as `qO1Az` fills it) and the column headings above the rows
//! (`YSXrS`). Selecting a row opens the skill in the editor.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::time::Timestamp;

use super::{super::ProjectNav, copy};
use crate::{
  display,
  navigation::route::Page,
  screens::project::repository_import::copy::file_count,
  skills::data::{ProjectSkills, SkillEntry},
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    icon::{Icon, IconSize, icon},
    message_text,
    pressable::surface,
    status_label::status_label,
  },
};

/// Column widths, the icon surface and the heading row height (`l13EYM`, `jbOc2`, `aVVO1`,
/// `Rbl80`, `ZnLJf`, `TEjWP`, `YSXrS`).
const ICON_WIDTH: f32 = 36.0;
const FILES_WIDTH: f32 = 56.0;
const VERSION_WIDTH: f32 = 180.0;
const SCOPE_WIDTH: f32 = 150.0;
const PUBLISHED_WIDTH: f32 = 88.0;
const OPEN_WIDTH: f32 = 14.0;
const HEADINGS_HEIGHT: f32 = 32.0;

fn cell(text: &str, width: f32, color: PaletteColor) -> Text {
  Text::new(text)
    .variant(TypographyStyle::Caption)
    .color(color)
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .width(Dimension::Px(width))
}

pub fn column_headings(ctx: &Ctx) -> Element {
  let heading = |key: &str, width: f32| cell(&ctx.t(key), width, PaletteColor::TextMuted);
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEADINGS_HEIGHT))
    // Inset like the rows, so the columns line up.
    .padding_horizontal(ExtraSpacing::Stack.role())
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Row::new().width(Dimension::Px(ICON_WIDTH)))
    .child(
      Text::new(&ctx.t("skills.library.columns.skill"))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextMuted)
        .flex(1.0),
    )
    .child(heading("skills.library.columns.files", FILES_WIDTH))
    .child(heading("skills.library.columns.version", VERSION_WIDTH))
    .child(heading("skills.library.columns.scope", SCOPE_WIDTH))
    .child(heading("skills.library.columns.lastPublished", PUBLISHED_WIDTH))
    .child(Row::new().width(Dimension::Px(OPEN_WIDTH)))
    .into()
}

pub fn skill_row(ctx: &Ctx, entry: &SkillEntry, data: &ProjectSkills, nav: &ProjectNav) -> Element {
  let icon_surface = Row::new()
    .size(Dimension::Px(ICON_WIDTH), Dimension::Px(ICON_WIDTH))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .background(PaletteColor::SurfaceRaised)
    .rounded(ExtraRadius::Notice.role())
    .child(icon(Icon::Sparkles, IconSize::Notice, PaletteColor::TextPrimary));
  let description = entry.body.description().trim();
  let description = if description.is_empty() {
    ctx.t("skills.library.noDescription")
  } else {
    description.into()
  };
  let identity = Column::new()
    .flex(1.0)
    .spacing(SpacingSize::Sm)
    .child(
      Text::new(&entry.name)
        .variant(ExtraTypography::MonoTitle.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .child(
      Text::new(&description)
        .variant(TypographyStyle::Caption)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    );
  let files = entry.body.files().len();
  let files = if files == 0 {
    ctx.t("skills.files.none")
  } else {
    file_count(ctx, files)
  };
  let glyph = if entry.state.has_draft() {
    Icon::Pencil
  } else {
    Icon::CircleCheck
  };
  let version = Row::new()
    .width(Dimension::Px(VERSION_WIDTH))
    .child(status_label(glyph, &copy::version(ctx, entry.state)));
  let published = entry.last_published.map_or_else(
    || ctx.t("skills.library.never").to_string(),
    |at| message_text(ctx, &display::elapsed(Timestamp::now(), at)).to_string(),
  );
  let row = surface()
    .id(format!("skill-row-{}", entry.slug))
    .describe("skill", entry.name.to_string())
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Md)
    .padding_horizontal(ExtraSpacing::Stack.role())
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .hovered_style(Style::new().background(PaletteColor::SurfaceRaised))
    .child(icon_surface)
    .child(identity)
    .child(cell(&files, FILES_WIDTH, PaletteColor::TextSecondary))
    .child(version)
    .child(cell(
      &copy::scope(ctx, entry.body.scope(), data, false),
      SCOPE_WIDTH,
      PaletteColor::TextSecondary,
    ))
    .child(cell(&published, PUBLISHED_WIDTH, PaletteColor::TextSecondary))
    .child(icon(Icon::ChevronRight, IconSize::Dismiss, PaletteColor::TextSecondary));
  nav.link(Page::Skills(Some(entry.id))).on(row).into()
}
