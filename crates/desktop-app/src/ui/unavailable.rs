//! Why something cannot be used. Something not built or not allowed right now stays in place, visibly
//! unavailable with a lock, and says why in one line (handover §5.6): on its own line (`Unavailable
//! reason` `QR1ER`: a 12 lock and the words in 11.5 `text_muted`, 6 apart), or under the actions it is
//! about, at their end (`Actions` `mQeh3`: 11 `text_muted`, 6 under the buttons).

use lurq::{
  app::theme::{PaletteColor, TypographyStyle},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::icon::{Icon, icon};
use crate::theme::{ExtraSpacing, ExtraTypography, IconSize};

/// The lock and `reason` on a line of their own, with the element id `id`: as wide as its parent, the
/// words wrapping there and the lock staying with the first line; the words' element id is `<id>-text`.
pub fn reason_line(id: &str, reason: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Start)
    .describe("role", "unavailable-reason")
    .child(icon(Icon::Lock, IconSize::Sm, PaletteColor::TextMuted).flex_shrink(0.0))
    .child(
      Text::new(reason)
        .id(format!("{id}-text"))
        .variant(ExtraTypography::Note.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}

/// `actions` with `reason` under them, both at their end; the reason's element id is `id`.
pub fn with_reason(actions: impl Into<Element>, id: &str, reason: &str) -> Column {
  Column::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::End)
    .child(actions)
    .child(
      Text::new(reason)
        .id(id.to_owned())
        .variant(TypographyStyle::Caption)
        .describe("role", "unavailable-reason"),
    )
}
