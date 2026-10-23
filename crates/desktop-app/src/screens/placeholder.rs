//! An icon, a title and a line under it in the content panel's middle: what a path that names nothing shows
//! (`not_found`) and a task form that cannot be shown.

use lurq::{
  app::theme::{PaletteColor, SpacingSize, TypographyStyle},
  components::{Column, Text},
  layout::{Alignment, layout_kind::Justify},
  node::dimension::Dimension,
};

use crate::{
  theme::IconSize,
  ui::icon::{Icon, icon},
};

/// An icon, a title and a line under it, in the middle of the content panel.
pub fn centered(glyph: Icon, title: &str, body: &str) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .spacing(SpacingSize::Sm)
    .child(icon(glyph, IconSize::Lg, PaletteColor::TextMuted))
    .child(Text::new(title).variant(TypographyStyle::Title))
    .child(Text::new(body).variant(TypographyStyle::Description))
}
