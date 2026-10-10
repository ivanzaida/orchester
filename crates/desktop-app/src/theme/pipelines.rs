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
  theme.set_palette_color(canvas_color(), lurq::node::color::Color::from_hex("#0D0D0F"));
  theme.set_radius_value(parallel_radius(), 12.0);
}
pub const GRAPH_WIDTH: f32 = 760.0;
pub const GRAPH_CENTER: f32 = 424.0;
pub const GRAPH_TOP: f32 = 36.0;
pub const EMPTY_TOP: f32 = 249.0;
pub const CARD_HEIGHT: f32 = 82.0;
pub const GATE_HEIGHT: f32 = 22.0;
pub const EDGE_HEIGHT: f32 = 22.0;
pub const PARALLEL_HEAD: f32 = 28.0;
pub const PARALLEL_FOOT: f32 = 12.0;
pub const CARD_GAP: f32 = 8.0;
pub fn canvas_color() -> lurq::app::theme::PaletteColor {
  lurq::app::theme::PaletteColor::extra("pipeline_canvas")
}
