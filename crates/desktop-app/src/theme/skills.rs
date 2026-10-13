//! Skills values from xF9B8, dIKkG and CrBUa.
use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};
/// TmfIL/ng3rc, f30K2 and XNgaG/l2Qrl0: compact mono section captions.
pub fn import_caption() -> TypographyStyle {
  TypographyStyle::extra("skills_import_caption")
}
/// TmfIL/OLgEc/XfKrO: repository path with a compact 16px line box.
pub fn import_path() -> TypographyStyle {
  TypographyStyle::extra("skills_import_path")
}
/// F8mJNd/NKAjl: 13px screen explanation, line height 1.45.
pub fn library_lead() -> TypographyStyle {
  TypographyStyle::extra("skills_library_lead")
}
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
pub fn file_inset() -> SpacingSize {
  SpacingSize::extra("skills_file_inset")
}
pub fn result_header_bottom() -> SpacingSize {
  SpacingSize::extra("skills_result_header_bottom")
}
pub fn result_spacing() -> SpacingSize {
  SpacingSize::extra("skills_result_spacing")
}
pub(super) fn install(theme: &Theme) {
  theme.set_spacing_value(file_inset(), 7.0);
  theme.set_spacing_value(result_header_bottom(), 18.0);
  theme.set_spacing_value(result_spacing(), 22.0);
  theme.set_spacing_value(source_line(), 20.0);
  theme.set_spacing_value(form_inset(), 28.0);
  theme.set_spacing_value(section_spacing(), 20.0);
  theme.set_spacing_value(cell_spacing(), 12.0);
}
pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  [
    (
      "skills_import_caption",
      10.5,
      FontWeight::Normal,
      1.3,
      true,
      palette.text_muted,
    ),
    (
      "skills_import_path",
      12.0,
      FontWeight::Normal,
      1.3,
      true,
      palette.text_primary,
    ),
    (
      "skills_library_lead",
      13.0,
      FontWeight::Normal,
      1.45,
      false,
      palette.text_secondary,
    ),
    (
      "skills_source",
      12.0,
      FontWeight::Normal,
      20.0 / 12.0,
      true,
      palette.text_primary,
    ),
    (
      "skills_gutter",
      11.0,
      FontWeight::Normal,
      20.0 / 11.0,
      true,
      palette.text_muted,
    ),
    ("skills_name", 13.0, FontWeight::Medium, 1.3, true, palette.text_primary),
    (
      "skills_heading",
      11.0,
      FontWeight::Medium,
      1.3,
      false,
      palette.text_muted,
    ),
  ]
  .into_iter()
  .map(|(name, size, weight, line, mono, color)| {
    let style = super::typography::style(size, weight, line, 0.0, color);
    (
      Arc::from(name),
      if mono { super::typography::mono(style) } else { style },
    )
  })
}
