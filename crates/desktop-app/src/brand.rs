//! The Orchester mark (`Brand / Orchester`, `OLHng`): the icon the window is created with (taskbar
//! button, Alt+Tab) and the mark in the title bar, drawn with the window's own anti-aliased rounded
//! shapes. The executable's icon is embedded by `build.rs`, and on Windows `system_icon` gives it to the
//! window and its class once the first frame is drawn; macOS takes an app's icon from its bundle and
//! ignores the window's.

use image::{ImageError, ImageFormat};
use lurq::{
  app::WindowIcon,
  components::Row,
  layout::{Alignment, layout_kind::Justify},
  node::dimension::Dimension,
};

use crate::theme::{ExtraColor, ExtraRadius, ExtraSpacing};

const ICON_PNG: &[u8] = include_bytes!("../assets/icon/orchester-256.png");
/// The mark's tile (`Logo` `Jbirl`), its bars' width and their heights, left to right.
const MARK_SIZE: f32 = 24.0;
const BAR_WIDTH: f32 = 3.5;
const BAR_HEIGHTS: [f32; 3] = [7.0, 13.0, 9.5];

/// The 256px frame of `orchester.ico`, for `WinitWindow::with_icon`.
pub fn window_icon() -> Result<WindowIcon, ImageError> {
  let image = image::load_from_memory_with_format(ICON_PNG, ImageFormat::Png)?.into_rgba8();
  let (width, height) = image.dimensions();
  Ok(WindowIcon::from_rgba(image.into_raw(), width, height))
}

/// The mark: three rounded bars on a dark tile, the same in both modes.
pub fn mark() -> Row {
  BAR_HEIGHTS.into_iter().fold(
    Row::new()
      .width(Dimension::Px(MARK_SIZE))
      .height(Dimension::Px(MARK_SIZE))
      .rounded(ExtraRadius::BrandTile.role())
      .background(ExtraColor::BrandTile.role())
      .spacing(ExtraSpacing::BrandBars.role())
      .align_items(Alignment::Center)
      .justify(Justify::Center),
    |tile, height| {
      tile.child(
        Row::new()
          .width(Dimension::Px(BAR_WIDTH))
          .height(Dimension::Px(height))
          .rounded(ExtraRadius::BrandBar.role())
          .background(ExtraColor::BrandMark.role()),
      )
    },
  )
}
