//! Pipeline-specific geometry from CpL4m, x4IIjM and qXE7Z.
use lurq::app::theme::{RadiusSize, SpacingSize, Theme};
pub fn page_inset() -> SpacingSize {
  SpacingSize::extra("pipeline_page_inset")
}
pub fn parallel_radius() -> RadiusSize {
  RadiusSize::extra("pipeline_parallel")
}
pub fn install(theme: &Theme) {
  theme.set_spacing_value(page_inset(), 40.0);
  theme.set_radius_value(parallel_radius(), 12.0);
}
