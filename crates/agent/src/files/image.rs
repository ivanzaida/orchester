//! The images `read_file` meets: a PNG, JPEG or WebP file, told by its content, never by its name. An agent CLI that
//! shows MCP images (Claude Code) gets the image itself ([`ShownImage`]): decoded within [`decoding_limits`],
//! downscaled so its long side is at most [`MAX_SENT_SIDE`] and re-encoded as PNG, which drops whatever metadata the
//! file carried. Other agents get a note that names the image and its size ([`ImageKind::note`]).

use std::{fmt, io::Cursor};

use image::{DynamicImage, ImageFormat, ImageReader, Limits, imageops::FilterType};
use thiserror::Error;

/// The longest side an image is sent at: larger ones are downscaled to it, keeping their aspect ratio.
pub const MAX_SENT_SIDE: u32 = 1568;
/// The most bytes of base64 one image answer holds.
pub const MAX_SENT_DATA: usize = 5 * 1024 * 1024;
/// The widest and the tallest image decoded.
const MAX_DECODED_SIDE: u32 = 8192;
/// The most memory one decode may take.
const MAX_DECODE_ALLOC: u64 = 128 * 1024 * 1024;

/// The limits every image Orchester decodes is read within: `read_file`'s images and the app's screenshots
/// `call_run_app` saves.
pub fn decoding_limits() -> Limits {
  let mut limits = Limits::default();
  limits.max_alloc = Some(MAX_DECODE_ALLOC);
  limits.max_image_width = Some(MAX_DECODED_SIDE);
  limits.max_image_height = Some(MAX_DECODED_SIDE);
  limits
}

/// The image formats `read_file` shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageKind {
  Png,
  Jpeg,
  Webp,
}

impl ImageKind {
  /// The format whose signature `bytes` start with.
  pub fn of(bytes: &[u8]) -> Option<Self> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
      Some(Self::Png)
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
      Some(Self::Jpeg)
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
      Some(Self::Webp)
    } else {
      None
    }
  }

  pub const fn name(self) -> &'static str {
    match self {
      Self::Png => "PNG",
      Self::Jpeg => "JPEG",
      Self::Webp => "WebP",
    }
  }

  const fn format(self) -> ImageFormat {
    match self {
      Self::Png => ImageFormat::Png,
      Self::Jpeg => ImageFormat::Jpeg,
      Self::Webp => ImageFormat::WebP,
    }
  }

  fn reader(self, bytes: &[u8]) -> ImageReader<Cursor<&[u8]>> {
    let mut reader = ImageReader::with_format(Cursor::new(bytes), self.format());
    reader.limits(decoding_limits());
    reader
  }

  /// What an agent that is not shown images reads for this image in `bytes`, `not_shown` saying who is not.
  pub(super) fn note(self, bytes: &[u8], not_shown: &str) -> String {
    let size = self.reader(bytes).into_dimensions().map_or_else(
      |_| "its size cannot be read".to_owned(),
      |size| Size::from(size).to_string(),
    );
    format!("This is a {} image ({size}). {not_shown}", self.name())
  }
}

/// An image's size in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
  pub width: u32,
  pub height: u32,
}

impl From<(u32, u32)> for Size {
  fn from((width, height): (u32, u32)) -> Self {
    Self { width, height }
  }
}

impl fmt::Display for Size {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}×{}", self.width, self.height)
  }
}

/// Why an image file is not sent. The message follows the file's path in the error result.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ImageError {
  #[error("it starts as a {kind} image but cannot be decoded: {detail}")]
  Undecodable { kind: &'static str, detail: String },
  #[error(
    "the image is still too large after downscaling: {bytes} bytes of base64 as a PNG at {size}, more than read_file \
     sends ({limit} bytes)",
    limit = MAX_SENT_DATA
  )]
  TooLarge { bytes: usize, size: Size },
}

/// An image as `read_file` sends it: re-encoded as PNG, at most [`MAX_SENT_SIDE`] on its long side.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShownImage {
  kind: ImageKind,
  original: Size,
  sent: Size,
  png: Vec<u8>,
}

impl ShownImage {
  /// The image of `kind` in `bytes`, decoded, downscaled and re-encoded.
  pub fn of(bytes: &[u8], kind: ImageKind) -> Result<Self, ImageError> {
    let undecodable = |error: image::ImageError| ImageError::Undecodable {
      kind: kind.name(),
      detail: error.to_string(),
    };
    let image = kind.reader(bytes).decode().map_err(undecodable)?;
    let original = Size::from((image.width(), image.height()));
    let image = downscaled(image);
    let sent = Size::from((image.width(), image.height()));
    let mut png = Vec::new();
    image
      .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
      .map_err(undecodable)?;
    let bytes = png.len().div_ceil(3) * 4;
    if bytes > MAX_SENT_DATA {
      return Err(ImageError::TooLarge { bytes, size: sent });
    }
    Ok(Self {
      kind,
      original,
      sent,
      png,
    })
  }

  /// The PNG the agent is sent.
  pub fn png(&self) -> &[u8] {
    &self.png
  }

  pub const fn original(&self) -> Size {
    self.original
  }

  pub const fn sent(&self) -> Size {
    self.sent
  }

  /// The line the agent reads beside the image, which its transcript row also keeps: the format and both sizes.
  pub fn line(&self) -> String {
    format!("{} read, {}, sent at {}", self.kind.name(), self.original, self.sent)
  }
}

/// `image`, scaled down so that its long side is at most [`MAX_SENT_SIDE`]; a smaller one as it is.
fn downscaled(image: DynamicImage) -> DynamicImage {
  if image.width().max(image.height()) <= MAX_SENT_SIDE {
    return image;
  }
  image.resize(MAX_SENT_SIDE, MAX_SENT_SIDE, FilterType::Lanczos3)
}
