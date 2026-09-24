//! Pipeline summary step (`XVkaB`): a step number beside the step's name and a short summary.

use lurq::{
  app::theme::{PaletteColor, TypographyStyle},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use crate::theme::{ExtraSpacing, ExtraTypography};

/// Row height (`XVkaB`).
const STEP_HEIGHT: f32 = 38.0;

pub fn summary_step(number: usize, title: &str, summary: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(STEP_HEIGHT))
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(&format!("{number:02}"))
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextMuted),
    )
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(title)
            .variant(TypographyStyle::FieldLabel)
            .color(PaletteColor::TextPrimary),
        )
        .child(Text::new(summary).variant(ExtraTypography::Footnote.role())),
    )
    .into()
}
