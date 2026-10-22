//! How much a message may carry, as Claude Desktop allows: a number of files, a size for each and a size for all of
//! them together; and when pasted text becomes an attachment instead of flooding the message.

use super::{AttachmentRefusal, NewAttachment};

/// The most attachments one message holds.
pub const MAX_ATTACHMENTS: usize = 10;
/// The largest file a message takes: 20 MB.
pub const MAX_ATTACHMENT_BYTES: u64 = 20 * 1024 * 1024;
/// The most a message's attachments hold together: 50 MB.
pub const MAX_MESSAGE_BYTES: u64 = 50 * 1024 * 1024;
/// Pasted text longer than this many characters becomes a text attachment.
pub const LONG_PASTE_CHARS: usize = 2_000;

/// Refuses a file of `size` bytes that is empty or larger than [`MAX_ATTACHMENT_BYTES`].
pub const fn check_size(size: u64) -> Result<(), AttachmentRefusal> {
  if size == 0 {
    return Err(AttachmentRefusal::Empty);
  }
  if size > MAX_ATTACHMENT_BYTES {
    return Err(AttachmentRefusal::TooLarge {
      limit: MAX_ATTACHMENT_BYTES,
    });
  }
  Ok(())
}

/// Refuses adding `new` to a message that holds `held`: past [`MAX_ATTACHMENTS`], or past [`MAX_MESSAGE_BYTES`]
/// together.
pub fn check_adding(held: &[NewAttachment], new: &NewAttachment) -> Result<(), AttachmentRefusal> {
  if held.len() >= MAX_ATTACHMENTS {
    return Err(AttachmentRefusal::TooMany { limit: MAX_ATTACHMENTS });
  }
  let total: u64 = held.iter().map(NewAttachment::size).sum();
  if total.saturating_add(new.size()) > MAX_MESSAGE_BYTES {
    return Err(AttachmentRefusal::TotalTooLarge {
      limit: MAX_MESSAGE_BYTES,
    });
  }
  Ok(())
}

/// Refuses a message with `attachments` that holds more than the limits allow, however it was put together.
pub fn check_message(attachments: &[NewAttachment]) -> Result<(), AttachmentRefusal> {
  for (at, attachment) in attachments.iter().enumerate() {
    check_size(attachment.size())?;
    check_adding(&attachments[..at], attachment)?;
  }
  Ok(())
}

/// Whether pasting `text` makes it an attachment rather than words of the message.
pub fn is_long_paste(text: &str) -> bool {
  text.chars().count() > LONG_PASTE_CHARS
}
