//! A chart's tooltip (`Chart tooltip` `G7cuH`): 216 wide on `surface_raised` inside a `border_strong`
//! stroke, radius 8, lifted by the `lg` shadow, 10 over and under its content and 12 at its sides, its
//! parts 8 apart: the title in 12/600, lines 5 apart (an 8 swatch where the line is a series, the name in 12
//! `text_secondary`, the exact figure in 12 mono at the end), and where it came from in 11 `text_muted`. It
//! sits beside what it describes, on its left where the right has no room.

use lurq::{
  app::theme::{BorderSize, PaletteColor, ShadowStyle, SpacingSize, TypographyStyle},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use super::card::{SWATCH, swatch};
use crate::theme::{ExtraRadius, ExtraSpacing, ExtraTypography};

/// The tooltip's width, and how far it keeps from what it describes.
const WIDTH: f32 = 216.0;
const OFFSET: f32 = 12.0;

/// The tooltip `id`: `title`, its `lines`, and where it came from.
pub fn tip(id: &str, title: &str, lines: Vec<Element>, meta: Option<&str>) -> Element {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Px(WIDTH))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceRaised)
    .border_inside(BorderSize::Sm, PaletteColor::BorderStrong)
    .rounded(ExtraRadius::Tile.role())
    .box_shadow(ShadowStyle::Lg)
    .child(
      Text::new(title)
        .id(format!("{id}-title"))
        .variant(ExtraTypography::InspectorSection.role()),
    )
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Pair.role())
        .with_children(lines),
    )
    .with_children(meta.map(|meta| {
      Text::new(meta)
        .id(format!("{id}-meta"))
        .width(Dimension::Pct(100.0))
        .variant(TypographyStyle::Caption)
    }))
    .into()
}

/// One line: a swatch where it is a series (the room of one where it is not), its name, its figure if any.
pub fn line(color: Option<PaletteColor>, name: &str, value: Option<&str>) -> Element {
  let mark = match color {
    Some(color) => swatch(color),
    None => Row::new().width(Dimension::Px(SWATCH)).flex_shrink(0.0).into(),
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(mark)
    .child(
      Text::new(name)
        .variant(TypographyStyle::Description)
        .nowrap()
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .with_children(value.map(|value| Text::new(value).variant(TypographyStyle::Mono).nowrap()))
    .into()
}

/// `tip` placed in a chart `width` wide beside what spans `left` to `right`: after it, or before it when
/// the room after it is too narrow.
pub fn beside(tip: Element, (left, right): (f32, f32), width: f32) -> Element {
  let after = right + OFFSET;
  let x = if after + WIDTH <= width {
    after
  } else {
    (left - OFFSET - WIDTH).max(0.0)
  };
  Column::new().absolute_position(x, 0.0).child(tip).into()
}
