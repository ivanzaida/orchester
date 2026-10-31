//! A screencast frame decoded for drawing: the runner's JPEG as RGBA pixels, decoded off the UI thread
//! (`docs/design/browser.md`, "The *Browser* window", the page).

use std::{fmt, sync::Arc};

use image::ImageFormat;
use orchester_ipc::browser::{BrowserTabId, ScreencastFrame};

/// One frame of the tab shown, ready to be drawn as one image: `width` × `height` device pixels, four bytes each
/// (red, green, blue, alpha), row by row from the top.
#[derive(Clone, PartialEq, Eq)]
pub struct RgbaFrame {
  tab: BrowserTabId,
  sequence: u64,
  width: u32,
  height: u32,
  css_width: u32,
  css_height: u32,
  pixels: Arc<[u8]>,
}

impl fmt::Debug for RgbaFrame {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("RgbaFrame")
      .field("tab", &self.tab)
      .field("sequence", &self.sequence)
      .field("width", &self.width)
      .field("height", &self.height)
      .field("css_width", &self.css_width)
      .field("css_height", &self.css_height)
      .finish_non_exhaustive()
  }
}

/// The runner's JPEG of a frame is not one.
#[derive(Debug, thiserror::Error)]
#[error("screencast frame {sequence} cannot be decoded: {source}")]
pub struct FrameDecodeError {
  sequence: u64,
  #[source]
  source: image::ImageError,
}

impl RgbaFrame {
  /// Decodes `frame`'s JPEG. Takes milliseconds for a full page: never on the UI thread.
  pub fn decode(frame: &ScreencastFrame) -> Result<Self, FrameDecodeError> {
    let decoded =
      image::load_from_memory_with_format(&frame.jpeg, ImageFormat::Jpeg).map_err(|source| FrameDecodeError {
        sequence: frame.sequence,
        source,
      })?;
    let rgba = decoded.into_rgba8();
    Ok(Self {
      tab: frame.tab.clone(),
      sequence: frame.sequence,
      width: rgba.width(),
      height: rgba.height(),
      css_width: frame.css_width,
      css_height: frame.css_height,
      pixels: rgba.into_raw().into(),
    })
  }

  pub fn tab(&self) -> &BrowserTabId {
    &self.tab
  }

  /// The runner's number of the frame: a later frame has a higher one.
  pub const fn sequence(&self) -> u64 {
    self.sequence
  }

  /// Its width in device pixels.
  pub const fn width(&self) -> u32 {
    self.width
  }

  /// Its height in device pixels.
  pub const fn height(&self) -> u32 {
    self.height
  }

  /// The page area it shows, in CSS pixels, where the person's pointer is placed.
  pub const fn css_size(&self) -> (u32, u32) {
    (self.css_width, self.css_height)
  }

  pub fn pixels(&self) -> &Arc<[u8]> {
    &self.pixels
  }
}
