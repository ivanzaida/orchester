//! Skills values from xF9B8, dIKkG and CrBUa.
use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};
pub fn source_line() -> SpacingSize {
  SpacingSize::extra("skills_source_line")
}
pub fn source() -> TypographyStyle {
  TypographyStyle::extra("skills_source")
}
pub fn gutter() -> TypographyStyle {
  TypographyStyle::extra("skills_gutter")
}
pub fn form_inset() -> SpacingSize {
  SpacingSize::extra("skills_form_inset")
}
pub fn name() -> TypographyStyle {
  TypographyStyle::extra("skills_name")
}
pub fn section_spacing() -> SpacingSize {
  SpacingSize::extra("skills_section")
}
pub fn cell_spacing() -> SpacingSize {
  SpacingSize::extra("skills_cell")
}
pub fn heading() -> TypographyStyle {
  TypographyStyle::extra("skills_heading")
}
pub(super) fn install(theme: &Theme) {
  theme.set_spacing_value(source_line(), 20.0);
  theme.set_spacing_value(form_inset(), 28.0);
  theme.set_spacing_value(section_spacing(), 20.0);
  theme.set_spacing_value(cell_spacing(), 12.0);
}
pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  [
    (
      Arc::from("skills_source"),
      super::typography::mono(super::typography::style(
        12.0,
        FontWeight::Normal,
        20.0 / 12.0,
        0.0,
        palette.text_primary,
      )),
    ),
    (
      Arc::from("skills_gutter"),
      super::typography::mono(super::typography::style(
        11.0,
        FontWeight::Normal,
        20.0 / 11.0,
        0.0,
        palette.text_muted,
      )),
    ),
    (
      Arc::from("skills_name"),
      super::typography::mono(super::typography::style(
        13.0,
        FontWeight::Medium,
        1.3,
        0.0,
        palette.text_primary,
      )),
    ),
    (
      Arc::from("skills_heading"),
      super::typography::style(11.0, FontWeight::Medium, 1.3, 0.0, palette.text_muted),
    ),
  ]
  .into_iter()
}
