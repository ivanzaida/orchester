//! Files attached to a message in the chats (ORC-57), as both composers and both conversations show them: a chip for
//! each — an image's thumbnail or the kind's icon, the file's name and size, and in a composer its remove cross
//! ([`chip`]); what a composer holds before its message is sent ([`draft`]); the composer's tray of chips with what
//! was refused, *Attach*, and the drop on its box ([`tray`]); a preview over the composer or under a message: a text
//! file's text, an image larger ([`preview`]); and the attachments of a sent message ([`sent`]). The design has no
//! frames for them yet: they follow the composer's and the chat's looks, and every value is a theme role.

pub mod chip;
pub mod draft;
pub mod preview;
pub mod sent;
pub mod tray;

use lurq::app::{component::DevtoolsInspectable, ctx::Ctx};
use orchester_core::{
  attachment::{AttachmentKind, MediaType},
  id::AttachmentId,
};

use super::icon::Icon;

/// The file whose preview is open, if any: a signal's value, which lurq's DevTools need marked (core's ids are not).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Opened(pub Option<AttachmentId>);

impl DevtoolsInspectable for Opened {}

impl Opened {
  /// Opens the preview of `id`, or closes it when it is the one open.
  pub fn toggle(&mut self, id: AttachmentId) {
    self.0 = if self.0 == Some(id) { None } else { Some(id) };
  }
}

/// The side of a chip's thumbnail or icon tile.
pub(crate) const TILE: f32 = 32.0;
/// The pixels a chip's thumbnail is decoded to, sharp on a 2x display.
pub(crate) const TILE_PIXELS: u32 = 64;
/// The widest a chip gets; its name is cut to fit.
pub(crate) const CHIP_WIDTH: f32 = 240.0;
/// The tallest an image's preview gets, and the pixels it is decoded to.
pub(crate) const PREVIEW_HEIGHT: f32 = 360.0;
pub(crate) const PREVIEW_PIXELS: u32 = 1600;

/// Bytes in a kilobyte, as sizes are said.
const KILOBYTE: u64 = 1024;

/// The size `bytes` as a person reads it: bytes, whole kilobytes, or megabytes with one decimal.
pub fn size_words(ctx: &Ctx, bytes: u64) -> String {
  let (key, size) = if bytes < KILOBYTE {
    ("attachments.size.bytes", bytes.to_string())
  } else if bytes < KILOBYTE * KILOBYTE {
    ("attachments.size.kilobytes", bytes.div_ceil(KILOBYTE).to_string())
  } else {
    let tenths = (bytes * 10).div_ceil(KILOBYTE * KILOBYTE);
    ("attachments.size.megabytes", format!("{}.{}", tenths / 10, tenths % 10))
  };
  ctx.t_args(key, [("size", size)]).to_string()
}

/// The icon of a file of `media` without a thumbnail.
pub const fn kind_icon(media: MediaType) -> Icon {
  match media.kind() {
    AttachmentKind::Image => Icon::FileImage,
    AttachmentKind::Pdf => Icon::File,
    AttachmentKind::Text => Icon::FileText,
  }
}

/// The key a thumbnail of attachment `id` is cached under: its id, so its content is never hashed again.
pub(crate) fn image_key(id: AttachmentId) -> lurq::images::ImageKey {
  let (high, low) = id.as_uuid().as_u64_pair();
  lurq::images::ImageKey::new(high ^ low)
}
