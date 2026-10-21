//! What a file is, told by its content as Claude Desktop does, never by its name: the signature of an image or a PDF,
//! and an image's size from its header; else UTF-8 text without a NUL byte. A Windows shortcut (`.lnk`) and a macOS
//! alias are told by theirs, so they are refused as shortcuts rather than as files of an unknown kind.

use super::{AttachmentRefusal, Dimensions, MediaType};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";
const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF];
const PDF: &[u8] = b"%PDF-";
/// A PDF's signature may follow some bytes of junk; readers look for it in the first kilobyte.
const PDF_WITHIN: usize = 1024;
/// The header of a Windows shell link.
const LNK: &[u8] = &[
  0x4C, 0x00, 0x00, 0x00, 0x01, 0x14, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
  0x46,
];
/// The header of a macOS alias (a bookmark file).
const ALIAS: &[u8] = b"book\0\0\0\0mark\0\0\0\0";
const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// What `content` is, with an image's size when its header gives it; refused when it is a shortcut or of a kind
/// Orchester does not take.
pub fn detect(content: &[u8]) -> Result<(MediaType, Option<Dimensions>), AttachmentRefusal> {
  if content.starts_with(LNK) || content.starts_with(ALIAS) {
    return Err(AttachmentRefusal::Shortcut);
  }
  if content.starts_with(PNG) {
    return Ok((MediaType::Png, png(content)));
  }
  if content.starts_with(JPEG) {
    return Ok((MediaType::Jpeg, jpeg(content)));
  }
  if content.starts_with(b"GIF87a") || content.starts_with(b"GIF89a") {
    return Ok((MediaType::Gif, gif(content)));
  }
  if content.len() >= 12 && &content[..4] == b"RIFF" && &content[8..12] == b"WEBP" {
    return Ok((MediaType::Webp, webp(content)));
  }
  let head = &content[..content.len().min(PDF_WITHIN)];
  if head.windows(PDF.len()).any(|window| window == PDF) {
    return Ok((MediaType::Pdf, None));
  }
  if is_text(content) {
    return Ok((MediaType::Text, None));
  }
  Err(AttachmentRefusal::Unsupported)
}

/// UTF-8, with or without a byte order mark, and no NUL byte, which no text file holds.
fn is_text(content: &[u8]) -> bool {
  let body = content.strip_prefix(UTF8_BOM).unwrap_or(content);
  !body.contains(&0) && std::str::from_utf8(body).is_ok()
}

fn be16(content: &[u8], at: usize) -> Option<u32> {
  Some(u32::from(u16::from_be_bytes(content.get(at..at + 2)?.try_into().ok()?)))
}

fn le16(content: &[u8], at: usize) -> Option<u32> {
  Some(u32::from(u16::from_le_bytes(content.get(at..at + 2)?.try_into().ok()?)))
}

fn le24(content: &[u8], at: usize) -> Option<u32> {
  let bytes = content.get(at..at + 3)?;
  Some(u32::from(bytes[0]) | (u32::from(bytes[1]) << 8) | (u32::from(bytes[2]) << 16))
}

fn sized(width: u32, height: u32) -> Option<Dimensions> {
  (width > 0 && height > 0).then_some(Dimensions { width, height })
}

/// The size in the `IHDR` chunk, which always comes first.
fn png(content: &[u8]) -> Option<Dimensions> {
  if content.get(12..16)? != b"IHDR" {
    return None;
  }
  let width = u32::from_be_bytes(content.get(16..20)?.try_into().ok()?);
  let height = u32::from_be_bytes(content.get(20..24)?.try_into().ok()?);
  sized(width, height)
}

/// The logical screen's size.
fn gif(content: &[u8]) -> Option<Dimensions> {
  sized(le16(content, 6)?, le16(content, 8)?)
}

/// The size in the first start-of-frame segment, walking the segments before it by their lengths.
fn jpeg(content: &[u8]) -> Option<Dimensions> {
  let mut at = 2;
  loop {
    while *content.get(at)? == 0xFF && *content.get(at + 1)? == 0xFF {
      at += 1;
    }
    if *content.get(at)? != 0xFF {
      return None;
    }
    let marker = *content.get(at + 1)?;
    let length = usize::try_from(be16(content, at + 2)?).ok()?;
    let start_of_frame = (0xC0..=0xCF).contains(&marker) && !matches!(marker, 0xC4 | 0xC8 | 0xCC);
    if start_of_frame {
      return sized(be16(content, at + 7)?, be16(content, at + 5)?);
    }
    if length < 2 {
      return None;
    }
    at += 2 + length;
  }
}

/// The size of a lossy (`VP8 `), lossless (`VP8L`) or extended (`VP8X`) WebP.
fn webp(content: &[u8]) -> Option<Dimensions> {
  match content.get(12..16)? {
    b"VP8 " => sized(le16(content, 26)? & 0x3FFF, le16(content, 28)? & 0x3FFF),
    b"VP8L" => {
      let bits = u32::from_le_bytes(content.get(21..25)?.try_into().ok()?);
      sized((bits & 0x3FFF) + 1, ((bits >> 14) & 0x3FFF) + 1)
    }
    b"VP8X" => sized(le24(content, 24)? + 1, le24(content, 27)? + 1),
    _ => None,
  }
}
