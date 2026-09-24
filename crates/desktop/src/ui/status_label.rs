//! Status label (`DeizF`): an icon and a word on the hover fill. Status always shows both.

use lurq::{
  app::theme::{PaletteColor, RadiusSize, SpacingSize},
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::icon::{Icon, IconSize, icon};
use crate::theme::{ExtraSpacing, ExtraTypography};

/// Label height (`DeizF`).
const LABEL_HEIGHT: f32 = 26.0;

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
