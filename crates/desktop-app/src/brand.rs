//! The Orchester mark (`Brand / Orchester`, `OLHng`): the window icon (taskbar button, Alt+Tab) and the
//! mark in the title bar. The executable's own icon is embedded by `build.rs`; macOS takes an app's icon
//! from its bundle and ignores the window's.

use image::{ImageError, ImageFormat};
use lurq::{app::WindowIcon, components::Svg, node::dimension::Dimension};

const MARK_SVG: &str = include_str!("../assets/brand/orchester-mark.svg");
const ICON_PNG: &[u8] = include_bytes!("../assets/icon/orchester-256.png");

/// The 256px frame of `orchester.ico`, for `WinitWindow::with_icon`.
pub fn window_icon() -> Result<WindowIcon, ImageError> {
  let image = image::load_from_memory_with_format(ICON_PNG, ImageFormat::Png)?.into_rgba8();
  let (width, height) = image.dimensions();
  Ok(WindowIcon::from_rgba(image.into_raw(), width, height))
}

/// The mark, `size` logical pixels square.
pub fn mark(size: f32) -> Svg {
  Svg::from_str(MARK_SVG)
    .width(Dimension::Px(size))
    .height(Dimension::Px(size))
}
