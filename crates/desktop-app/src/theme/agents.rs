//! Agents page padding from NPPl4 and a7PAf.
use lurq::app::theme::{SpacingSize, Theme};
pub fn page_spacing() -> SpacingSize {
  SpacingSize::extra("agents_page")
}
pub(super) fn install(theme: &Theme) {
  theme.set_spacing_value(page_spacing(), 40.0);
}
