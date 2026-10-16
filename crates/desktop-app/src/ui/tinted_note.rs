//! A note in one color (the Orchestrator's cards, `Action card` `nQIhx`): text in 11.5 beside its 12 icon in the same
//! color, 6 apart, the icon at the first line. The app says with it why a person-only press did nothing: the hand in
//! `warning` ([`person_only`]).

use lurq::{
  app::theme::PaletteColor,
  components::{Row, Text},
  layout::Alignment,
  node::dimension::Dimension,
};

use super::icon::{Icon, icon};
use crate::theme::{ExtraSpacing, ExtraTypography, IconSize};

/// `text` in `color` beside `glyph` in the same color.
pub fn tinted_note(id: impl Into<String>, (glyph, color): (Icon, PaletteColor), text: &str) -> Row {
  Row::new()
    .id(id.into())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Start)
    .child(icon(glyph, IconSize::Sm, color.clone()))
    .child(
      Text::new(text)
        .variant(ExtraTypography::Note.role())
        .color(color)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}

/// Why a press did nothing that only a person may do here, as the Orchestrator says it: the hand in `warning`.
pub fn person_only(id: impl Into<String>, text: &str) -> Row {
  tinted_note(id, (Icon::Hand, PaletteColor::Warning), text)
}
