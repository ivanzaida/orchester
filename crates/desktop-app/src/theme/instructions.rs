//! Instructions fePhH: the editor, library and variable catalog's type and spacing.
use std::sync::Arc;

use lurq::{
  app::theme::{PaletteColor, RadiusSize, SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle, VerticalAlign},
};

/// Prompt body cCFTK: baseline inset below the toolbar.
pub fn prompt_top() -> SpacingSize {
  SpacingSize::extra("instructions_prompt_top")
}
pub fn head_top() -> SpacingSize {
  SpacingSize::extra("instructions_head_top")
}
pub fn empty_top() -> SpacingSize {
  SpacingSize::extra("instructions_empty_top")
}
/// T1w9U starts 32 below the actions: the empty state's 14 gap plus 18.
pub fn discovery_offset() -> SpacingSize {
  SpacingSize::extra("instructions_discovery_offset")
}
pub fn discovery_path() -> TypographyStyle {
  TypographyStyle::extra("instructions_discovery_path")
}
pub fn discovery_detail() -> TypographyStyle {
  TypographyStyle::extra("instructions_discovery_detail")
}
pub fn discovery_note() -> TypographyStyle {
  TypographyStyle::extra("instructions_discovery_note")
}
pub fn prompt_text() -> TypographyStyle {
  TypographyStyle::extra("instructions_prompt")
}
/// U69ob source lines retain their position, including leading and trailing blank lines.
pub fn source_text() -> TypographyStyle {
  TypographyStyle::extra("instructions_source")
}
/// x5WE6i: mono variable names and values keep a 16/17px line rhythm.
pub fn variable_name() -> TypographyStyle {
  TypographyStyle::extra("instructions_variable_name")
}
pub fn detail_value() -> TypographyStyle {
  TypographyStyle::extra("instructions_detail_value")
}
pub fn detail_description() -> TypographyStyle {
  TypographyStyle::extra("instructions_detail_description")
}
/// nPUFf: the full viewer's outlined Esc keycap.
pub fn viewer_keycap() -> TypographyStyle {
  TypographyStyle::extra("instructions_viewer_keycap")
}
pub fn variable_value() -> TypographyStyle {
  TypographyStyle::extra("instructions_variable_value")
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
  theme.set_spacing_value(prompt_top(), 16.0);
  theme.set_spacing_value(empty_top(), 56.0);
  theme.set_spacing_value(token_padding(), 3.0);
  theme.set_radius_value(token_radius(), 3.0);
  theme.set_spacing_value(discovery_offset(), 18.0);
}
pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  use super::typography::{mono, style};
  [
    (
      Arc::from("instructions_viewer_keycap"),
      mono(style(10.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted)),
    ),
    (
      Arc::from("instructions_variable_name"),
      mono(style(12.0, FontWeight::Normal, 4.0 / 3.0, 0.0, palette.text_primary)),
    ),
    (
      Arc::from("instructions_detail_value"),
      mono(style(12.0, FontWeight::Normal, 1.45, 0.0, palette.text_primary)),
    ),
    (
      Arc::from("instructions_detail_description"),
      style(12.0, FontWeight::Normal, 1.45, 0.0, palette.text_secondary),
    ),
    (
      Arc::from("instructions_variable_value"),
      mono(style(11.0, FontWeight::Normal, 1.4, 0.0, palette.text_muted)),
    ),
    (
      Arc::from("instructions_prompt"),
      mono(style(12.0, FontWeight::Normal, 1.666667, 0.0, palette.text_secondary)),
    ),
    (
      Arc::from("instructions_source"),
      TextStyle {
        vertical_align: VerticalAlign::LineBox,
        ..mono(style(12.0, FontWeight::Normal, 1.666667, 0.0, palette.text_primary))
      },
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
    // T1w9U: the rows' default 16px lines, and the 15px footer line.
    (
      Arc::from("instructions_discovery_path"),
      mono(style(12.0, FontWeight::Normal, 4.0 / 3.0, 0.0, palette.text_primary)),
    ),
    (
      Arc::from("instructions_discovery_detail"),
      style(12.0, FontWeight::Normal, 4.0 / 3.0, 0.0, palette.text_secondary),
    ),
    (
      Arc::from("instructions_discovery_note"),
      style(11.5, FontWeight::Normal, 15.0 / 11.5, 0.0, palette.text_muted),
    ),
  ]
  .into_iter()
}
