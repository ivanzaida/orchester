//! Read-only repository preview TmfIL/NrdFj; content and candidates come from workspace.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::agent_files::import::{Candidate, ImportItem, ItemKind};

use crate::{
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::icon::{Icon, icon},
};
const KEY_WIDTH: f32 = 120.0;
const HEAD_HEIGHT: f32 = 32.0;
pub fn panel(ctx: &Ctx, item: &ImportItem) -> Element {
  let mut panel = Column::new()
    .id("skill-import-preview")
    .flex(1.0)
    .height(Dimension::Pct(100.0))
    .min_width(Dimension::Px(0.0))
    .padding(Padding::symmetric(SpacingSize::Lg, SpacingSize::Md))
    .spacing(ExtraSpacing::Inset.role())
    .child(
      Text::new(&ctx.t_args("skills.preview", [("source", item.source.as_str())]))
        .variant(ExtraTypography::ColumnHeading.role()),
    );
  panel = panel.child(details(ctx, item));
  let filename = if item.kind == ItemKind::Skill {
    "SKILL.md"
  } else {
    item.source.as_str()
  };
  let content = item.text.as_deref().unwrap_or_default();
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEAD_HEIGHT))
    .padding(Padding::horizontal(crate::theme::skills::cell_spacing()))
    .spacing(SpacingSize::Sm)
    .child(icon(
      Icon::FileText,
      crate::theme::IconSize::Md,
      PaletteColor::TextMuted,
    ))
    .child(Text::new(filename).variant(TypographyStyle::Mono).flex(1.0))
    .child(
      Text::new(&ctx.t_args(
        "skills.previewFileMeta",
        [("lines", content.lines().count().to_string())],
      ))
      .variant(ExtraTypography::Note.role()),
    );
  panel
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .min_height(Dimension::Px(0.0))
        .border_inside(BorderSize::Sm, PaletteColor::Border)
        .rounded(ExtraRadius::Tile.role())
        .background(PaletteColor::SurfaceInput)
        .clip()
        .child(head)
        .child(
          ScrollVertical::new(
            Text::new(content)
              .id("skill-import-content")
              .variant(TypographyStyle::Mono)
              .selectable(true)
              .width(Dimension::Pct(100.0))
              .padding(crate::theme::skills::cell_spacing()),
          )
          .width(Dimension::Pct(100.0))
          .flex(1.0),
        ),
    )
    .into()
}
fn property(ctx: &Ctx, key: &str, value: &str) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(crate::theme::skills::cell_spacing())
    .child(
      Text::new(&ctx.t(key))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .width(Dimension::Px(KEY_WIDTH)),
    )
    .child(Text::new(value).variant(TypographyStyle::Description).flex(1.0))
}
fn details(ctx: &Ctx, item: &ImportItem) -> Element {
  if let Ok(candidate) = &item.outcome {
    let kind = match candidate {
      Candidate::Skill { .. } => "skills.skill",
      Candidate::Instruction { .. } => "skills.instruction",
    };
    let mut details = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(property(
        ctx,
        "skills.becomes",
        &ctx.t_args(
          "skills.previewDraft",
          [
            ("kind", ctx.t(kind).to_string()),
            ("name", candidate.name().to_string()),
          ],
        ),
      ))
      .child(property(ctx, "skills.scope", &ctx.t("skills.previewScope")));
    if let Candidate::Skill { description, files, .. } = candidate {
      details = details
        .child(property(ctx, "skills.previewDescription", description))
        .child(property(
          ctx,
          "skills.previewFiles",
          &if files.is_empty() {
            ctx.t("skills.none").to_string()
          } else {
            files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>().join(" · ")
          },
        ));
    }
    details.into()
  } else {
    Element::from(Text::new(&super::repository::item_summary(ctx, item)).variant(TypographyStyle::Description))
  }
}
