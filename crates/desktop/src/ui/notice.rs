//! Inline notice (`q5LzI`): an icon beside a short heading and its detail.

use lurq::{
  app::theme::{PaletteColor, SpacingSize, TypographyStyle},
  components::{Column, Row, Text},
  node::{Element, dimension::Dimension},
};

use super::icon::{Icon, IconSize, icon};
use crate::theme::{ExtraRadius, ExtraSpacing, ExtraTypography};

pub fn notice(glyph: Icon, heading: &str, detail: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .spacing(ExtraSpacing::Stack.role())
    .background(PaletteColor::SurfaceRaised)
    .rounded(ExtraRadius::Notice.role())
    .child(icon(glyph, IconSize::Feature, PaletteColor::TextSecondary))
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(heading)
            .variant(TypographyStyle::FieldLabel)
            .color(PaletteColor::TextPrimary),
        )
        .child(Text::new(detail).variant(ExtraTypography::Detail.role())),
    )
    .into()
}
