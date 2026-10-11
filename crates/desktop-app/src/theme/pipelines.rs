//! Pipeline-specific geometry from CpL4m, x4IIjM and qXE7Z.
use lurq::app::theme::{RadiusSize, SpacingSize, Theme};
pub fn page_inset() -> SpacingSize {
  SpacingSize::extra("pipeline_page_inset")
}
pub fn parallel_radius() -> RadiusSize {
  RadiusSize::extra("pipeline_parallel")
}
pub fn return_radius() -> RadiusSize {
  RadiusSize::extra("pipeline_return")
}
pub fn return_stroke() -> lurq::app::theme::BorderSize {
  lurq::app::theme::BorderSize::extra("pipeline_return")
}
pub fn install(theme: &Theme) {
  theme.set_radius_value(return_radius(), 8.0);
  theme.set_border_size_value(return_stroke(), 1.5);
  theme.set_spacing_value(page_inset(), 40.0);
  theme.set_palette_color(canvas_color(), lurq::node::color::Color::from_hex("#0D0D0F"));
  theme.set_radius_value(parallel_radius(), 12.0);
}
pub const GRAPH_WIDTH: f32 = 852.0;
pub const GRAPH_CENTER: f32 = 470.0;
pub const GRAPH_TOP: f32 = 36.0;
pub const EMPTY_TOP: f32 = 249.0;
pub const CARD_HEIGHT: f32 = 82.0;
pub const GATE_HEIGHT: f32 = 22.0;
pub const EDGE_HEIGHT: f32 = 22.0;
pub const PARALLEL_HEAD: f32 = 28.0;
pub const PARALLEL_FOOT: f32 = 12.0;
pub const CARD_GAP: f32 = 12.0;
pub fn canvas_color() -> lurq::app::theme::PaletteColor {
  lurq::app::theme::PaletteColor::extra("pipeline_canvas")
}
/// v1nZD's dashed outline; explicit segments preserve dashes in lurq's SVG tessellator.
pub fn slot_outline() -> &'static str {
  r#"<svg xmlns="http://www.w3.org/2000/svg" width="240" height="74" viewBox="0 0 240 74"><path d="M0.5 8 A7.5 7.5 0 0 1 8 0.5 M232 0.5 A7.5 7.5 0 0 1 239.5 8 M239.5 66 A7.5 7.5 0 0 1 232 73.5 M8 73.5 A7.5 7.5 0 0 1 0.5 66 M8 0.5h4 M8 73.5h4 M16 0.5h4 M16 73.5h4 M24 0.5h4 M24 73.5h4 M32 0.5h4 M32 73.5h4 M40 0.5h4 M40 73.5h4 M48 0.5h4 M48 73.5h4 M56 0.5h4 M56 73.5h4 M64 0.5h4 M64 73.5h4 M72 0.5h4 M72 73.5h4 M80 0.5h4 M80 73.5h4 M88 0.5h4 M88 73.5h4 M96 0.5h4 M96 73.5h4 M104 0.5h4 M104 73.5h4 M112 0.5h4 M112 73.5h4 M120 0.5h4 M120 73.5h4 M128 0.5h4 M128 73.5h4 M136 0.5h4 M136 73.5h4 M144 0.5h4 M144 73.5h4 M152 0.5h4 M152 73.5h4 M160 0.5h4 M160 73.5h4 M168 0.5h4 M168 73.5h4 M176 0.5h4 M176 73.5h4 M184 0.5h4 M184 73.5h4 M192 0.5h4 M192 73.5h4 M200 0.5h4 M200 73.5h4 M208 0.5h4 M208 73.5h4 M216 0.5h4 M216 73.5h4 M224 0.5h4 M224 73.5h4 M0.5 8v4 M239.5 8v4 M0.5 16v4 M239.5 16v4 M0.5 24v4 M239.5 24v4 M0.5 32v4 M239.5 32v4 M0.5 40v4 M239.5 40v4 M0.5 48v4 M239.5 48v4 M0.5 56v4 M239.5 56v4 M0.5 64v4 M239.5 64v4" fill="none" stroke="black" stroke-width="1"/></svg>"#
}
