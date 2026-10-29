//! Message framing: a 4-byte big-endian header followed by a body. The header's lower 31 bits are the body's length;
//! its top bit marks a binary frame (protocol 33, `docs/design/browser.md`, decision 13).
//!
//! - A JSON frame's body is one JSON message.
//! - A binary frame's body is a 4-byte big-endian length of a JSON head, the head, then raw bytes: a screencast frame's
//!   JPEG travels so, without the third more that base64 in JSON would add.
//!
//! A length prefix rather than newline-delimited JSON lets the reader reject an oversized
//! frame before allocating for it and read the body with one exact read instead of scanning
//! for a delimiter. JSON keeps frames readable in logs and captures, and serde_json is already
//! a workspace dependency.

use std::io::ErrorKind;

use serde::{Serialize, de::DeserializeOwned};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::FrameError;

/// Largest accepted frame body, of either kind. Frames carry commands, events, single output lines and screencast
/// frames, far below this; a larger length means a corrupt stream or a peer that is not an Orchester process.
pub const MAX_FRAME_LEN: u32 = 16 * 1024 * 1024;

const HEADER_LEN: usize = 4;
/// The header bit of a binary frame; [`MAX_FRAME_LEN`] leaves it clear in every JSON frame's length.
const BINARY: u32 = 1 << 31;

/// One frame as read: its JSON message, and the raw bytes a binary frame carries after it.
#[derive(Debug, PartialEq, Eq)]
pub struct Frame<T> {
  pub message: T,
  pub bytes: Option<Vec<u8>>,
}

/// Reads frames from one direction of a connection.
pub struct FrameReader<R> {
  inner: R,
}

impl<R: AsyncRead + Unpin> FrameReader<R> {
  pub fn new(inner: R) -> Self {
    Self { inner }
  }

  /// The next message, or `None` when the peer closed the connection between frames. A binary frame is refused
  /// here: only a reader that takes bytes reads one ([`FrameReader::read_frame`]).
  pub async fn read<T: DeserializeOwned>(&mut self) -> Result<Option<T>, FrameError> {
    match self.read_frame().await? {
      None => Ok(None),
      Some(Frame { message, bytes: None }) => Ok(Some(message)),
      Some(Frame { bytes: Some(_), .. }) => Err(FrameError::Malformed("a binary frame where only JSON is read")),
    }
  }

  /// The next frame of either kind, or `None` when the peer closed the connection between frames.
  pub async fn read_frame<T: DeserializeOwned>(&mut self) -> Result<Option<Frame<T>>, FrameError> {
    let Some(header) = self.read_header().await? else {
      return Ok(None);
    };
    let header = u32::from_be_bytes(header);
    let len = header & !BINARY;
    if len > MAX_FRAME_LEN {
      return Err(FrameError::TooLarge {
        len: len as usize,
        max: MAX_FRAME_LEN,
      });
    }
    let mut body = vec![0; len as usize];
    self.inner.read_exact(&mut body).await.map_err(|error| {
      if is_closed(error.kind()) {
        FrameError::Truncated
      } else {
        FrameError::Io(error)
      }
    })?;
    if header & BINARY == 0 {
      let message = serde_json::from_slice(&body).map_err(FrameError::Decode)?;
      return Ok(Some(Frame { message, bytes: None }));
    }
    let (head, bytes) = split_binary(body)?;
    let message = serde_json::from_slice(&head).map_err(FrameError::Decode)?;
    Ok(Some(Frame {
      message,
      bytes: Some(bytes),
    }))
  }

  async fn read_header(&mut self) -> Result<Option<[u8; HEADER_LEN]>, FrameError> {
    let mut header = [0; HEADER_LEN];
    let mut filled = 0;
    while filled < HEADER_LEN {
      let read = match self.inner.read(&mut header[filled..]).await {
        Ok(read) => read,
        Err(error) if is_closed(error.kind()) => 0,
        Err(error) => return Err(FrameError::Io(error)),
      };
      if read == 0 {
        return if filled == 0 {
          Ok(None)
        } else {
          Err(FrameError::Truncated)
        };
      }
      filled += read;
    }
    Ok(Some(header))
  }
}

/// Writes frames to one direction of a connection.
pub struct FrameWriter<W> {
  inner: W,
}

impl<W: AsyncWrite + Unpin> FrameWriter<W> {
  pub fn new(inner: W) -> Self {
    Self { inner }
  }

  /// Writes `message` as a JSON frame.
  pub async fn write<T: Serialize>(&mut self, message: &T) -> Result<(), FrameError> {
    let body = serde_json::to_vec(message).map_err(FrameError::Encode)?;
    self.send(0, &[&body]).await
  }

  /// Writes `message` with `bytes` after it as a binary frame; refused, with nothing written, when both do not fit
  /// [`MAX_FRAME_LEN`].
  pub async fn write_with_bytes<T: Serialize>(&mut self, message: &T, bytes: &[u8]) -> Result<(), FrameError> {
    let head = serde_json::to_vec(message).map_err(FrameError::Encode)?;
    let head_len = u32::try_from(head.len()).map_err(|_| FrameError::TooLarge {
      len: head.len(),
      max: MAX_FRAME_LEN,
    })?;
    self.send(BINARY, &[&head_len.to_be_bytes(), &head, bytes]).await
  }

  /// Sends one frame whose body is `parts` in order, with `kind`'s bit in its header.
  async fn send(&mut self, kind: u32, parts: &[&[u8]]) -> Result<(), FrameError> {
    let body_len: usize = parts.iter().map(|part| part.len()).sum();
    let len = u32::try_from(body_len)
      .ok()
      .filter(|len| *len <= MAX_FRAME_LEN)
      .ok_or(FrameError::TooLarge {
        len: body_len,
        max: MAX_FRAME_LEN,
      })?;
    let mut frame = Vec::with_capacity(HEADER_LEN + body_len);
    frame.extend_from_slice(&(len | kind).to_be_bytes());
    for part in parts {
      frame.extend_from_slice(part);
    }
    self.inner.write_all(&frame).await?;
    self.inner.flush().await?;
    Ok(())
  }
}

/// A binary frame's body as its JSON head and its bytes.
fn split_binary(mut body: Vec<u8>) -> Result<(Vec<u8>, Vec<u8>), FrameError> {
  let Some(head_len) = body.get(..HEADER_LEN) else {
    return Err(FrameError::Malformed("a binary frame without the length of its head"));
  };
  let head_len = u32::from_be_bytes(head_len.try_into().expect("four bytes were taken")) as usize;
  let head_end = HEADER_LEN
    .checked_add(head_len)
    .filter(|end| *end <= body.len())
    .ok_or(FrameError::Malformed(
      "a binary frame whose head is longer than its body",
    ))?;
  let bytes = body.split_off(head_end);
  body.drain(..HEADER_LEN);
  Ok((body, bytes))
}

/// A named pipe reports a closed peer as a broken pipe rather than end of file.
fn is_closed(kind: ErrorKind) -> bool {
  matches!(kind, ErrorKind::UnexpectedEof | ErrorKind::BrokenPipe)
}
