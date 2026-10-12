//! Instructions fePhH: the editor, library and variable catalog's type and spacing.
use lurq::{
  app::theme::{PaletteColor, RadiusSize, SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};
use std::sync::Arc;

pub fn head_top() -> SpacingSize {
  SpacingSize::extra("instructions_head_top")
}
pub fn empty_top() -> SpacingSize {
  SpacingSize::extra("instructions_empty_top")
}
pub fn prompt_text() -> TypographyStyle {
  TypographyStyle::extra("instructions_prompt")
}
pub fn gutter_text() -> TypographyStyle {
  TypographyStyle::extra("instructions_gutter")
}
pub fn rule_name() -> TypographyStyle {
  TypographyStyle::extra("instructions_rule_name")
}
pub fn overline() -> TypographyStyle {
  TypographyStyle::extra("instructions_overline")
}

/// Pill geometry of oA91D and k3WFT.
pub fn token_padding() -> SpacingSize {
  SpacingSize::extra("instructions_token_padding")
}
pub fn token_radius() -> RadiusSize {
  RadiusSize::extra("instructions_token_radius")
}
pub fn token_style(unavailable: bool) -> lurq::components::InputTokenStyle {
  let (text, fill) = if unavailable {
    (PaletteColor::Warning, PaletteColor::WarningMuted)
  } else {
    (PaletteColor::TextPrimary, super::ExtraColor::Pressed.role())
  };
  lurq::components::InputTokenStyle::new(text, fill)
    .padding(token_padding())
    .radius(token_radius())
    .typography(prompt_text())
}

pub(super) fn install(theme: &Theme) {
  // p6Bsv: 18 above the page heading.
  theme.set_spacing_value(head_top(), 18.0);
  theme.set_spacing_value(empty_top(), 56.0);
  theme.set_spacing_value(token_padding(), 3.0);
  theme.set_radius_value(token_radius(), 3.0);
}
pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  use super::typography::{mono, style};
  [
    (
      Arc::from("instructions_prompt"),
      mono(style(12.0, FontWeight::Normal, 1.666667, 0.0, palette.text_secondary)),
    ),
    (
      Arc::from("instructions_gutter"),
      mono(style(11.0, FontWeight::Normal, 20.0 / 11.0, 0.0, palette.text_muted)),
    ),
    (
      Arc::from("instructions_rule_name"),
      style(12.5, FontWeight::Medium, 1.3, 0.0, palette.text_primary),
    ),
    (
      Arc::from("instructions_overline"),
      mono(style(10.5, FontWeight::Normal, 1.4, 0.6, palette.text_muted)),
    ),
  ]
  .into_iter()
}
