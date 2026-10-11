//! v1nZD: dashed add-step slot, one target for its icon, title and hint.
use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Column, Row, Stack, Svg, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, HitTestBehavior, dimension::Dimension},
  svg::SvgData,
};

use super::{
  icon::{Icon, icon},
  pressable::pressable,
};
use crate::theme::{ExtraSpacing, ExtraTypography, IconSize};

pub const WIDTH: f32 = 240.0;
pub const HEIGHT: f32 = 74.0;

pub fn add_slot(ctx: &Ctx, id: &str, title: &str, hint: &str, action: impl Fn() + Send + Sync + 'static) -> Element {
  let outline = Svg::new(
    SvgData::from_str(crate::theme::pipelines::slot_outline())
      .with_stroke(ctx.theme().palette_color(PaletteColor::BorderStrong)),
  )
  .size(Dimension::Px(WIDTH), Dimension::Px(HEIGHT))
  .hit_test(HitTestBehavior::None);
  pressable(action)
    .id(id.to_owned())
    .child(
      Stack::new()
        .width(Dimension::Px(WIDTH))
        .height(Dimension::Px(HEIGHT))
        .child(outline)
        .child(
          Column::new()
            .width(Dimension::Pct(100.0))
            .height(Dimension::Pct(100.0))
            .align_items(Alignment::Center)
            .justify(Justify::Center)
            .spacing(ExtraSpacing::Snug.role())
            .child(
              Row::new()
                .spacing(ExtraSpacing::Snug.role())
                .align_items(Alignment::Center)
                .child(icon(Icon::Plus, IconSize::Md, PaletteColor::TextSecondary))
                .child(Text::new(title).variant(ExtraTypography::Strong.role())),
            )
            .child(Text::new(hint).variant(ExtraTypography::Note.role())),
        ),
    )
    .into()
}
