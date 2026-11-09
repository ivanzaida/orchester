//! An element's part of its window's capture: the capture is in pixels, the element's rectangle in the desktop's
//! units — pixels on Windows, points on macOS, where one point is the capture's width over the window's in pixels.

use super::{NativeCapture, NativeUiError, NativeWindow};

/// The part of `whole`, the capture of `window`, that the desktop rectangle `(left, top, right, bottom)` covers.
pub(super) fn crop(
  whole: &NativeCapture,
  window: &NativeWindow,
  (left, top, right, bottom): (i32, i32, i32, i32),
) -> Result<NativeCapture, NativeUiError> {
  let scale = |size: u32, units: i32| {
    if units > 0 {
      f64::from(size) / f64::from(units)
    } else {
      1.0
    }
  };
  let (across, down) = (
    scale(whole.width, window.bounds.width),
    scale(whole.height, window.bounds.height),
  );
  // A coordinate of the desktop as a column or row of the capture, within it.
  let within = |value: i32, origin: i32, scale: f64, size: u32| {
    let pixels = (f64::from(value - origin) * scale).round().clamp(0.0, f64::from(size));
    // Clamped to 0..=size above, so it fits.
    pixels as u32
  };
  let (x0, x1) = (
    within(left, window.bounds.x, across, whole.width),
    within(right, window.bounds.x, across, whole.width),
  );
  let (y0, y1) = (
    within(top, window.bounds.y, down, whole.height),
    within(bottom, window.bounds.y, down, whole.height),
  );
  if x1 <= x0 || y1 <= y0 {
    return Err(NativeUiError::Capture(
      "the element shows no part of itself in its window".to_owned(),
    ));
  }
  let (width, height) = (x1 - x0, y1 - y0);
  let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
  for row in y0..y1 {
    let start = (row as usize * whole.width as usize + x0 as usize) * 4;
    rgba.extend_from_slice(&whole.rgba[start..start + width as usize * 4]);
  }
  Ok(NativeCapture { width, height, rgba })
}
