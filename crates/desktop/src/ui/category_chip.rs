//! Category chip (`EfQuh`): what a request is, as its icon and name on `$raised` with a `$border`
//! outline — a command's category (or that it reads credentials), or the MCP server it calls.

use lurq::{
  app::theme::{BorderSize, PaletteColor, SpacingSize},
  components::{Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};

use super::icon::{Icon, IconSize, icon};
use crate::theme::{ExtraRadius, ExtraSpacing, ExtraTypography};

/// Chip height (`EfQuh`).
const HEIGHT: f32 = 22.0;

pub fn category_chip(id: &'static str, glyph: Icon, label: &str) -> Element {
  Row::new()
    .id(id)
    .height(Dimension::Px(HEIGHT))
    .padding_horizontal(SpacingSize::Sm)
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Chip.role())
    .child(icon(glyph, IconSize::Small, PaletteColor::TextPrimary))
    .child(Text::new(label).variant(ExtraTypography::StatusLabel.role()).nowrap())
    .into()
}
