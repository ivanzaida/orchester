//! The application icon: the window icon decodes, and the packed `orchester.ico` and
//! `orchester.icns` hold exactly the committed PNGs, so a PNG changed without running
//! `tools/app-icon.ps1` fails here.

use std::{fs, path::PathBuf};

use orchester_desktop::app_icon::window_icon;

fn icon_file(relative: &str) -> Vec<u8> {
  let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("assets/icon")
    .join(relative);
  fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn u16_le(bytes: &[u8], at: usize) -> usize {
  usize::from(u16::from_le_bytes([bytes[at], bytes[at + 1]]))
}

fn u32_le(bytes: &[u8], at: usize) -> usize {
  u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as usize
}

fn u32_be(bytes: &[u8], at: usize) -> usize {
  u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as usize
}

#[test]
fn app_icon_window_icon_is_the_256px_tile() {
  let icon = window_icon().expect("the bundled PNG decodes");
  assert_eq!((icon.width(), icon.height()), (256, 256));
  let alpha = |x: usize, y: usize| icon.rgba()[(y * 256 + x) * 4 + 3];
  assert_eq!(alpha(0, 0), 0, "the rounded corner is transparent");
  assert_eq!(alpha(128, 128), 255, "the tile is opaque");
}

#[test]
fn app_icon_ico_holds_the_windows_pngs() {
  let ico = icon_file("orchester.ico");
  assert_eq!((u16_le(&ico, 0), u16_le(&ico, 2)), (0, 1), "an icon file");
  let sizes = [16, 24, 32, 48, 64, 256];
  assert_eq!(u16_le(&ico, 4), sizes.len());
  for (index, size) in sizes.into_iter().enumerate() {
    let entry = 6 + 16 * index;
    assert_eq!(usize::from(ico[entry]), size % 256, "width of entry {index}");
    assert_eq!(usize::from(ico[entry + 1]), size % 256, "height of entry {index}");
    let (length, offset) = (u32_le(&ico, entry + 8), u32_le(&ico, entry + 12));
    assert_eq!(
      ico[offset..offset + length],
      icon_file(&format!("windows/orchester-{size}.png")),
      "{size}px image"
    );
  }
}

#[test]
fn app_icon_icns_holds_the_iconset_pngs() {
  let icns = icon_file("orchester.icns");
  assert_eq!(&icns[..4], b"icns");
  assert_eq!(u32_be(&icns, 4), icns.len());
  let expected = [
    ("icp4", "icon_16x16.png"),
    ("ic11", "icon_16x16@2x.png"),
    ("icp5", "icon_32x32.png"),
    ("ic12", "icon_32x32@2x.png"),
    ("ic07", "icon_128x128.png"),
    ("ic13", "icon_128x128@2x.png"),
    ("ic08", "icon_256x256.png"),
    ("ic14", "icon_256x256@2x.png"),
    ("ic09", "icon_512x512.png"),
    ("ic10", "icon_512x512@2x.png"),
  ];
  let mut at = 8;
  for (kind, file) in expected {
    let length = u32_be(&icns, at + 4);
    assert_eq!(&icns[at..at + 4], kind.as_bytes());
    assert_eq!(
      icns[at + 8..at + length],
      icon_file(&format!("orchester.iconset/{file}")),
      "{file}"
    );
    at += length;
  }
  assert_eq!(at, icns.len(), "no other entries");
}
