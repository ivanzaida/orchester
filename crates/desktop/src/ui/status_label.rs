//! Status label (`DeizF`): an icon and a word on the hover fill. Status always shows both.

use lurq::{
  app::theme::{PaletteColor, RadiusSize, SpacingSize},
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::icon::{Icon, IconSize, icon};
use crate::theme::{ExtraSpacing, ExtraTypography};

/// Label height (`DeizF`), and of the compact label in the pipeline topbar (`pLCWI`).
const LABEL_HEIGHT: f32 = 26.0;
const COMPACT_HEIGHT: f32 = 22.0;

pub fn status_label(glyph: Icon, label: &str) -> Element {
  Row::new()
    .height(Dimension::Px(LABEL_HEIGHT))
    .padding_horizontal(SpacingSize::Sm)
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::AccentMuted)
    .rounded(RadiusSize::Sm)
    .child(icon(glyph, IconSize::Small, PaletteColor::TextPrimary))
    .child(Text::new(label).variant(ExtraTypography::StatusLabel.role()).nowrap())
    .into()
}

/// The compact form beside a pipeline's name (`pLCWI`): smaller text, and the icon only where
/// the design shows one (a published version).
pub fn compact_status_label(glyph: Option<Icon>, label: &str, id: &'static str) -> Element {
  let mut row = Row::new()
    .id(id)
    .height(Dimension::Px(COMPACT_HEIGHT))
    .padding_horizontal(SpacingSize::Sm)
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::AccentMuted)
    .rounded(RadiusSize::Sm);
  if let Some(glyph) = glyph {
    row = row.child(icon(glyph, IconSize::Small, PaletteColor::TextPrimary));
  }
  row
    .child(
      Text::new(label)
        .variant(ExtraTypography::StatusLabelSmall.role())
        .nowrap(),
    )
    .into()
}
