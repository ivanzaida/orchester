//! What the cards share: the frame (`surface_raised`, radius 10, a `warning` stroke while it waits and a
//! `border` one once decided, `tKYgO` and its answered state `I6cZ5j`), the head (a glyph, the title and,
//! at the end, where it stands: a 12 glyph and 11.5 words 5 apart, `pCqIo`) and a note line.

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};

use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::icon::{Icon, icon},
};

/// A card's frame: raised, its stroke `warning` while `waiting`, `border` once decided.
pub(super) fn frame(id: String, waiting: bool) -> Column {
  Column::new()
    .id(id)
    .describe("waiting", waiting.to_string())
    .width(Dimension::Pct(100.0))
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(
      BorderSize::Sm,
      if waiting {
        PaletteColor::Warning
      } else {
        PaletteColor::Border
      },
    ))
    .rounded(RadiusSize::Lg)
}

/// Where a card stands, at the end of its head: its glyph and words in `color`.
pub(super) struct Stands {
  pub(super) glyph: Icon,
  pub(super) words: std::sync::Arc<str>,
  pub(super) color: PaletteColor,
}

/// The head: `glyph` in `color`, `title` in `role`, then where the card stands at the end.
pub(super) fn head(
  id: &str,
  (glyph, size, color): (Icon, IconSize, PaletteColor),
  (title, role): (&str, TypographyStyle),
  stands: Element,
) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, size, color))
    .child(
      Text::new(title)
        .id(format!("{id}-title"))
        .variant(role)
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(Spacer::new().flex(1.0))
    .child(stands)
}

impl Stands {
  pub(super) fn waiting(words: std::sync::Arc<str>) -> Self {
    Self {
      glyph: Icon::Hand,
      words,
      color: PaletteColor::Warning,
    }
  }

  pub(super) fn into_element(self, id: &str) -> Element {
    Row::new()
      .id(format!("{id}-state"))
      .flex_shrink(0.0)
      .spacing(ExtraSpacing::Pair.role())
      .align_items(Alignment::Center)
      .child(icon(self.glyph, IconSize::Sm, self.color.clone()))
      .child(
        Text::new(&self.words)
          .variant(ExtraTypography::Note.role())
          .color(self.color)
          .nowrap(),
      )
      .into()
  }
}

/// An 11.5 `text_muted` line, `id`: when it was asked, what an answer does, what automation may do.
pub(super) fn note(id: &str, words: &str) -> Text {
  Text::new(words)
    .id(id.to_owned())
    .variant(ExtraTypography::Note.role())
    .width(Dimension::Pct(100.0))
}
