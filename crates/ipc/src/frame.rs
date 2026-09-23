//! Message framing: a 4-byte big-endian body length followed by a JSON body.
//!
//! A length prefix rather than newline-delimited JSON lets the reader reject an oversized
//! frame before allocating for it and read the body with one exact read instead of scanning
//! for a delimiter. JSON keeps frames readable in logs and captures, and serde_json is already
//! a workspace dependency.

use std::io::ErrorKind;

use serde::{Serialize, de::DeserializeOwned};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::FrameError;

/// Largest accepted frame body. Frames carry commands, events and single output lines, far
/// below this; a larger length means a corrupt stream or a peer that is not an Orchester process.
pub const MAX_FRAME_LEN: u32 = 16 * 1024 * 1024;

const HEADER_LEN: usize = 4;

/// Reads frames from one direction of a connection.
pub struct FrameReader<R> {
  inner: R,
}

impl<R: AsyncRead + Unpin> FrameReader<R> {
  pub fn new(inner: R) -> Self {
    Self { inner }
  }

  /// The next message, or `None` when the peer closed the connection between frames.
  pub async fn read<T: DeserializeOwned>(&mut self) -> Result<Option<T>, FrameError> {
    let Some(header) = self.read_header().await? else {
      return Ok(None);
    };
    let len = u32::from_be_bytes(header);
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
    serde_json::from_slice(&body).map(Some).map_err(FrameError::Decode)
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

  pub async fn write<T: Serialize>(&mut self, message: &T) -> Result<(), FrameError> {
    let body = serde_json::to_vec(message).map_err(FrameError::Encode)?;
    let len = u32::try_from(body.len())
      .ok()
      .filter(|len| *len <= MAX_FRAME_LEN)
      .ok_or(FrameError::TooLarge {
        len: body.len(),
        max: MAX_FRAME_LEN,
      })?;
    let mut frame = Vec::with_capacity(HEADER_LEN + body.len());
    frame.extend_from_slice(&len.to_be_bytes());
    frame.extend_from_slice(&body);
    self.inner.write_all(&frame).await?;
    self.inner.flush().await?;
    Ok(())
  }
}

/// A named pipe reports a closed peer as a broken pipe rather than end of file.
fn is_closed(kind: ErrorKind) -> bool {
  matches!(kind, ErrorKind::UnexpectedEof | ErrorKind::BrokenPipe)
}
