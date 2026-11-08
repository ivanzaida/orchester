//! *What stopped it*, or for a merged run *What happened* (`What stopped it` in 13a `i0K3xB`): a strip under the status
//! line, 12 inside its `border` outline (radius 6, 10 over and under), a 14 glyph in the status' color, then the title
//! in 12/600 and the words of the stage that ended the last attempt in 12 `text_secondary`, 2 under it.

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::{AttemptText, ExtraSpacing, ExtraTypography, IconSize},
  ui::icon::{Icon, icon},
};

/// The strip under `id`: `glyph` in `color`, `title` and `words`.
pub(super) fn strip(id: &str, (glyph, color): (Icon, PaletteColor), (title, words): (&str, &str)) -> Element {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Loose.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .align_items(Alignment::Start)
    .child(
      Row::new()
        .height(Dimension::Px(ICON_BOX))
        .align_items(Alignment::Center)
        .child(icon(glyph, IconSize::Md, color)),
    )
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(title)
            .id(format!("{id}-title"))
            .variant(ExtraTypography::InspectorSection.role())
            .width(Dimension::Pct(100.0)),
        )
        .child(
          Text::new(words)
            .id("merge-attempt-explanation")
            .variant(AttemptText::Explanation.role())
            .width(Dimension::Pct(100.0)),
        ),
    )
    .into()
}

/// The glyph's box, as high as the title's line (`Icon box`).
const ICON_BOX: f32 = 18.0;
