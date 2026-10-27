//! A stage chip (`Stage chip` `hDyRI`), as the delivery's phases line them up: its state's icon, its number and its
//! name in a pill.

use lurq::{
  app::theme::{BorderSize, PaletteColor},
  components::{Row, Text},
  layout::Alignment,
  node::{border::Border, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize},
  ui::icon::{Icon, icon},
};

/// A chip's height (`hDyRI`).
const CHIP_HEIGHT: f32 = 24.0;

/// A chip: its glyph in its color, its number when it has one, and its name, in a pill of `stroke`.
pub fn stage_chip(
  id: &str,
  (number, name): (Option<&str>, &str),
  ((glyph, color), name_color, stroke): ((Icon, PaletteColor), PaletteColor, PaletteColor),
) -> Row {
  Row::new()
    .id(id.to_owned())
    .height(Dimension::Px(CHIP_HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Inline.role()))
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .border(Border::inside(BorderSize::Sm, stroke))
    .rounded(ExtraRadius::Full.role())
    .child(icon(glyph, IconSize::Sm, color))
    .with_children(number.map(|number| Text::new(number).variant(ExtraTypography::MonoNote.role())))
    .child(
      Text::new(name)
        .variant(ExtraTypography::Note.role())
        .color(name_color)
        .nowrap(),
    )
}
