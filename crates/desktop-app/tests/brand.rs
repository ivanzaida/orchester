//! The Orchester mark as the app's icon: the window icon decodes, and `orchester.ico`, which `build.rs`
//! embeds in the executable, holds the sizes Windows draws, with the window icon as its 256px frame.

use std::{fs, path::PathBuf};

use image::ImageFormat;
use orchester_app::brand;

/// The frames of `orchester.ico`, in order: the sizes Windows picks from for 100 to 200 % displays
/// (title, thumbnail, taskbar, Alt+Tab, Explorer).
const ICO_SIZES: [u32; 6] = [16, 24, 32, 48, 64, 256];

fn icon_file(name: &str) -> Vec<u8> {
  let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/icon").join(name);
  fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn u16_le(bytes: &[u8], at: usize) -> usize {
  usize::from(u16::from_le_bytes([bytes[at], bytes[at + 1]]))
}

fn u32_le(bytes: &[u8], at: usize) -> usize {
  u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as usize
}

#[test]
fn the_window_icon_decodes_at_256_pixels() {
  let icon = brand::window_icon().expect("the bundled window icon is a valid PNG");
  assert_eq!((icon.width(), icon.height()), (256, 256));
  assert_eq!(icon.rgba().len(), 256 * 256 * 4);
  let alpha = |x: usize, y: usize| icon.rgba()[(y * 256 + x) * 4 + 3];
  assert_eq!(alpha(0, 0), 0, "the rounded corner is transparent");
  assert_eq!(alpha(128, 128), 255, "the tile is opaque");
}

#[test]
fn the_ico_holds_the_standard_sizes_of_the_mark() {
  let ico = icon_file("orchester.ico");
  assert_eq!((u16_le(&ico, 0), u16_le(&ico, 2)), (0, 1), "an icon file");
  assert_eq!(u16_le(&ico, 4), ICO_SIZES.len());
  for (index, size) in ICO_SIZES.into_iter().enumerate() {
    let entry = 6 + 16 * index;
    // A directory entry stores 256 as 0.
    let stored = |at: usize| match ico[at] {
      0 => 256,
      side => u32::from(side),
    };
    assert_eq!((stored(entry), stored(entry + 1)), (size, size), "entry {index}");
    let (length, offset) = (u32_le(&ico, entry + 8), u32_le(&ico, entry + 12));
    let frame = &ico[offset..offset + length];
    let image = image::load_from_memory_with_format(frame, ImageFormat::Png)
      .unwrap_or_else(|error| panic!("the {size}px frame is a PNG: {error}"));
    assert_eq!((image.width(), image.height()), (size, size), "{size}px frame");
  }
}

#[test]
fn the_ico_256_frame_is_the_window_icon() {
  let ico = icon_file("orchester.ico");
  let last = 6 + 16 * (ICO_SIZES.len() - 1);
  let (length, offset) = (u32_le(&ico, last + 8), u32_le(&ico, last + 12));
  assert_eq!(ico[offset..offset + length], icon_file("orchester-256.png"));
}
