//! `read_file` and images: Orchester's `read_file` for Claude Code sends a PNG, JPEG or WebP file (told by its content)
//! as a PNG without metadata, at most 1568 px on its long side, and says so in a line; an image still too large is an
//! error. Every other agent reads a note that names the image. Other binary files, the scope and read stamps stay.

use std::{fs, io::Cursor};

use image::{DynamicImage, ImageFormat, RgbImage, RgbaImage};
use orchester_agent::files::{
  ReadAnswer,
  image::{MAX_SENT_SIDE, ShownImage, Size},
};
use orchester_core::orchester_tools::{FileCall, ReadLines};
use serde_json::json;

use crate::{Layout, failed, line_edits::file_call, ok, read_input, text};

fn read(filename: &str) -> ReadLines {
  match file_call(
    "read_file",
    &json!({ "filename": filename, "start_line": 5, "length": 1 }),
  ) {
    FileCall::Read(read) => read,
    other => panic!("{other:?}"),
  }
}

/// A `width` × `height` image in `format`, a gradient.
fn encoded(width: u32, height: u32, format: ImageFormat) -> Vec<u8> {
  let image = RgbImage::from_fn(width, height, |x, y| {
    image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
  });
  let mut bytes = Vec::new();
  DynamicImage::ImageRgb8(image)
    .write_to(&mut Cursor::new(&mut bytes), format)
    .expect("encoded");
  bytes
}

fn shown(answer: ReadAnswer) -> ShownImage {
  match answer {
    ReadAnswer::Image(image) => image,
    ReadAnswer::Text(outcome) => panic!("expected an image, got: {}", outcome.content),
  }
}

fn answered_text(answer: ReadAnswer) -> orchester_core::native::session::ToolOutcome {
  match answer {
    ReadAnswer::Text(outcome) => outcome,
    ReadAnswer::Image(image) => panic!("expected text, got {}", image.line()),
  }
}

fn decoded(image: &ShownImage) -> DynamicImage {
  image::load_from_memory_with_format(image.png(), ImageFormat::Png).expect("a PNG")
}

/// `png` with a `tEXt` chunk holding `keyword` and `value` right after its header.
fn with_text_chunk(png: &[u8], keyword: &str, value: &str) -> Vec<u8> {
  let data = [keyword.as_bytes(), &[0], value.as_bytes()].concat();
  let mut chunk = u32::try_from(data.len()).expect("short").to_be_bytes().to_vec();
  let typed = [b"tEXt".as_slice(), &data].concat();
  chunk.extend_from_slice(&typed);
  chunk.extend_from_slice(&crc32(&typed).to_be_bytes());
  // The signature (8 bytes) and the IHDR chunk (25 bytes) come first.
  [&png[..33], &chunk, &png[33..]].concat()
}

fn crc32(bytes: &[u8]) -> u32 {
  let mut crc = u32::MAX;
  for byte in bytes {
    crc ^= u32::from(*byte);
    for _ in 0..8 {
      crc = if crc & 1 == 1 {
        (crc >> 1) ^ 0xedb8_8320
      } else {
        crc >> 1
      };
    }
  }
  !crc
}

#[test]
fn files_image_png_is_sent_as_a_png_with_both_sizes_and_without_metadata() {
  let layout = Layout::temporary();
  let png = with_text_chunk(&encoded(40, 20, ImageFormat::Png), "Comment", "token-in-metadata");
  fs::write(layout.worktree.join("shot.png"), &png).expect("file");
  let tools = layout.tools();
  let image = shown(tools.read_shown(&read("shot.png")));
  assert_eq!(image.line(), "PNG read, 40×20, sent at 40×20");
  let pixels = decoded(&image);
  assert_eq!((pixels.width(), pixels.height()), (40, 20));
  let sent = String::from_utf8_lossy(image.png());
  assert!(!sent.contains("token-in-metadata") && !sent.contains("tEXt"));
  // The read is stamped: an edit of the file is judged on its lines, not refused as unread.
  let edit = file_call(
    "edit_file",
    &json!({"filename": "shot.png", "start_line": 1, "end_line": 1, "expected": "x", "content": "y"}),
  );
  let FileCall::Edit(edit) = edit else { panic!("{edit:?}") };
  let refused = failed(tools.edit_lines(&edit));
  assert!(!refused.contains("has not been read"), "{refused}");
}

#[test]
fn files_image_large_is_downscaled_to_the_long_side() {
  let layout = Layout::temporary();
  fs::write(layout.worktree.join("wide.png"), encoded(3000, 1500, ImageFormat::Png)).expect("file");
  fs::write(layout.worktree.join("tall.png"), encoded(1000, 2000, ImageFormat::Png)).expect("file");
  let tools = layout.tools();
  let wide = shown(tools.read_shown(&read("wide.png")));
  assert_eq!(wide.line(), "PNG read, 3000×1500, sent at 1568×784");
  assert_eq!(
    wide.sent(),
    Size {
      width: MAX_SENT_SIDE,
      height: 784
    }
  );
  let pixels = decoded(&wide);
  assert_eq!((pixels.width(), pixels.height()), (1568, 784));
  let tall = shown(tools.read_shown(&read("tall.png")));
  assert_eq!(tall.line(), "PNG read, 1000×2000, sent at 784×1568");
}

#[test]
fn files_image_over_the_cap_after_downscaling_is_an_error() {
  let layout = Layout::temporary();
  // Noise does not compress: 1400 × 1400 pixels stay about 5.9 MB as a PNG, 7.8 MB as base64.
  let mut state = 0x2545_f491_u32;
  let noise = RgbaImage::from_fn(1400, 1400, |_, _| {
    state ^= state << 13;
    state ^= state >> 17;
    state ^= state << 5;
    image::Rgba(state.to_le_bytes())
  });
  let mut png = Vec::new();
  DynamicImage::ImageRgba8(noise)
    .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
    .expect("encoded");
  fs::write(layout.worktree.join("noise.png"), png).expect("file");
  let error = failed(answered_text(layout.tools().read_shown(&read("noise.png"))));
  assert!(
    error.starts_with("noise.png: the image is still too large after downscaling") && error.contains("at 1400×1400"),
    "{error}"
  );
}

#[test]
fn files_image_is_told_by_content_not_by_name() {
  let layout = Layout::temporary();
  fs::write(layout.worktree.join("photo.bin"), encoded(30, 10, ImageFormat::Jpeg)).expect("file");
  fs::write(layout.worktree.join("picture.dat"), encoded(12, 24, ImageFormat::WebP)).expect("file");
  fs::write(layout.worktree.join("fake.png"), "not an image\n").expect("file");
  let tools = layout.tools();
  assert_eq!(
    shown(tools.read_shown(&read("photo.bin"))).line(),
    "JPEG read, 30×10, sent at 30×10"
  );
  assert_eq!(
    shown(tools.read_shown(&read("picture.dat"))).line(),
    "WebP read, 12×24, sent at 12×24"
  );
  // Not an image: read as text, from line 1, since the file has one line and start_line 5 is past it.
  let fake = ok(answered_text(tools.read_shown(&read("fake.png"))));
  assert_eq!(fake, "the file has 1 lines; start_line 5 is past its end");
  let damaged = [b"\x89PNG\r\n\x1a\n".as_slice(), b"garbage"].concat();
  fs::write(layout.worktree.join("damaged.png"), damaged).expect("file");
  let error = failed(answered_text(tools.read_shown(&read("damaged.png"))));
  assert!(
    error.starts_with("damaged.png: it starts as a PNG image but cannot be decoded"),
    "{error}"
  );
}

#[test]
fn files_image_reads_keep_the_scope_and_binary_files() {
  let layout = Layout::temporary();
  fs::write(layout.outside.join("outside.png"), encoded(4, 4, ImageFormat::Png)).expect("file");
  fs::write(layout.worktree.join("blob.bin"), [1_u8, 0, 2, 3]).expect("file");
  let tools = layout.tools();
  let outside = failed(answered_text(
    tools.read_shown(&read(&text(&layout.outside.join("outside.png")))),
  ));
  assert!(outside.contains("outside the folders this step may read"), "{outside}");
  assert_eq!(
    ok(answered_text(tools.read_shown(&read("blob.bin")))),
    "binary file, 4 bytes"
  );
}

#[test]
fn files_image_is_only_named_for_agents_not_shown_images() {
  let layout = Layout::temporary();
  fs::write(layout.worktree.join("shot.png"), encoded(2882, 1875, ImageFormat::Png)).expect("file");
  let tools = layout.tools();
  let native = tools.read_file(&read_input("shot.png", None, None));
  assert_eq!(native.activity, ok(native.clone()));
  assert_eq!(
    ok(native),
    "This is a PNG image (2882×1875). Images are not shown to native agents yet."
  );
  assert_eq!(
    ok(tools.read_lines(&read("shot.png"))),
    "This is a PNG image (2882×1875). Images are shown on Claude Code connections only."
  );
}
