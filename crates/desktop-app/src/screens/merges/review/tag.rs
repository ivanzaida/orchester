//! A tag of the review page (`Tag · 2 comments` `GSgiW`, `Tag · Not sent` `I1Fsj`, `Tag · Sent to Implement`
//! `tew15`): its words in 11/500, an 11 glyph before them when it has one, 4 apart, 8 at its sides and 2 over and under
//! them, on its muted fill, round.

use lurq::{
  app::theme::{PaletteColor, SpacingSize},
  components::{Row, Text},
  layout::Alignment,
  node::{Element, padding::Padding},
};

use crate::{
  theme::{ExtraRadius, ExtraSpacing, IconSize, ReviewText},
  ui::icon::{Icon, icon},
};

/// How a tag looks: its fill, the color of its words and glyph, and its glyph.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct TagLook {
  pub fill: PaletteColor,
  pub color: PaletteColor,
  pub glyph: Option<Icon>,
}

impl TagLook {
  pub const fn new(fill: PaletteColor, color: PaletteColor) -> Self {
    Self {
      fill,
      color,
      glyph: None,
    }
  }

  pub const fn glyph(mut self, glyph: Icon) -> Self {
    self.glyph = Some(glyph);
    self
  }
}

/// The tag `id` saying `words`.
pub(super) fn tag(id: &str, words: &str, look: TagLook) -> Element {
  Row::new()
    .id(id.to_owned())
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Tight.role()))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .background(look.fill)
    .rounded(ExtraRadius::Full.role())
    .with_children(look.glyph.map(|glyph| icon(glyph, IconSize::Xs, look.color.clone())))
    .child(
      Text::new(words)
        .id(format!("{id}-text"))
        .variant(ReviewText::Tag.role())
        .color(look.color)
        .nowrap(),
    )
    .into()
}
