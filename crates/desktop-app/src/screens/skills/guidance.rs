//! Skills guidance x1SHz/owUF3: the three stages, with their outlined tiles and arrows.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  node::{Element, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::icon::{Icon, icon, marker},
};

pub fn how(ctx: &Ctx, vertical: bool) -> Element {
  let mut items = Vec::<Element>::new();
  for (index, (mark, title, body)) in [
    (Icon::Target, "skills.howScopeTitle", "skills.howScope"),
    (Icon::Upload, "skills.howPublishTitle", "skills.howPublish"),
    (Icon::Pin, "skills.howKeepTitle", "skills.howKeep"),
  ]
  .into_iter()
  .enumerate()
  {
    if index > 0 {
      items.push(
        Row::new()
          .width(Dimension::Px(if vertical { 24.0 } else { 13.0 }))
          .height(Dimension::Px(if vertical { 12.0 } else { 24.0 }))
          .align_items(lurq::layout::Alignment::Center)
          .justify(lurq::layout::layout_kind::Justify::Center)
          .child(icon(
            if vertical { Icon::ArrowDown } else { Icon::ArrowRight },
            if vertical { IconSize::Sm } else { IconSize::Base },
            PaletteColor::TextMuted,
          ))
          .into(),
      );
    }
    let stage = Row::new()
      .width(if vertical {
        Dimension::Pct(100.0)
      } else {
        Dimension::Auto
      })
      .spacing(ExtraSpacing::Loose.role())
      .child(marker(mark, 24.0))
      .child(
        Column::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .spacing(ExtraSpacing::Fine.role())
          .child(
            Text::new(&ctx.t(title))
              .width(Dimension::Pct(100.0))
              .variant(ExtraTypography::Strong.role()),
          )
          .child(
            Text::new(&ctx.t(body))
              .width(Dimension::Pct(100.0))
              .variant(ExtraTypography::Note.role()),
          ),
      );
    items.push(if vertical {
      stage.into()
    } else {
      stage.flex(1.0).min_width(Dimension::Px(0.0)).into()
    });
  }
  let title = Text::new(&ctx.t("skills.how")).variant(ExtraTypography::Strong.role());
  if vertical {
    Column::new()
      .id("skills-guidance")
      .width(Dimension::Pct(100.0))
      .padding(SpacingSize::Md)
      .spacing(SpacingSize::Sm)
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .rounded(crate::theme::ExtraRadius::Tile.role())
      .child(title)
      .with_children(items)
      .into()
  } else {
    Row::new()
      .id("skills-guidance")
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Inset.role()))
      .spacing(ExtraSpacing::Inset.role())
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .rounded(crate::theme::ExtraRadius::Tile.role())
      .child(Column::new().width(Dimension::Px(170.0)).child(title))
      .with_children(items)
      .into()
  }
}
