//! The application icon of the window (taskbar button, Alt+Tab), from the Pencil artwork
//! "App icon" (`Dzics`) bundled in `assets/icon`.
//!
//! Windows shows it for the running window; the executable's own icon (Explorer, pins, the Start
//! menu) is embedded by `build.rs`. macOS ignores a window icon and takes the icon of the `.app`
//! bundle instead (`docs/app-icon.md`).

use image::{ImageError, ImageFormat};
use lurq::app::WindowIcon;

const WINDOW_ICON_PNG: &[u8] = include_bytes!("../assets/icon/windows/orchester-256.png");

/// Decodes the bundled 256px icon for `WinitWindow::with_icon`.
pub fn window_icon() -> Result<WindowIcon, ImageError> {
  let image = image::load_from_memory_with_format(WINDOW_ICON_PNG, ImageFormat::Png)?.into_rgba8();
  let (width, height) = image.dimensions();
  Ok(WindowIcon::from_rgba(image.into_raw(), width, height))
}
