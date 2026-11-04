//! Screencast frames on the wire: [`crate::Event::BrowserFrame`] is written as a binary frame, its JPEG after its JSON
//! head (`docs/design/browser.md`, decision 13), and so is the reply that carries a run's file's bytes
//! ([`crate::run_files`]). Every other event is a JSON frame.

use std::fmt;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};

use super::BrowserTabId;
use crate::{
  Event, FrameError, Reply,
  frame::{Frame, FrameReader, FrameWriter},
  run_files::{FileBytes, RunFileRead, RunFilesReply},
};

/// One frame of the tab shown, as the browser encoded it: a JPEG of the page at the viewport's device pixels.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreencastFrame {
  pub tab: BrowserTabId,
  /// Counts the frames of the runner's screencasts: a gap means frames were dropped rather than queued.
  pub sequence: u64,
  /// The page area the frame shows, in CSS pixels.
  pub css_width: u32,
  pub css_height: u32,
  /// Travels after the JSON head, not in it.
  #[serde(skip)]
  pub jpeg: Vec<u8>,
}

impl fmt::Debug for ScreencastFrame {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("ScreencastFrame")
      .field("tab", &self.tab)
      .field("sequence", &self.sequence)
      .field("css_width", &self.css_width)
      .field("css_height", &self.css_height)
      .field("jpeg_bytes", &self.jpeg.len())
      .finish()
  }
}

impl<W: AsyncWrite + Unpin> FrameWriter<W> {
  /// Writes `event`: a screencast frame as a binary frame with its JPEG, a run's file read as a binary frame with its
  /// bytes, any other event as a JSON frame.
  pub async fn write_event(&mut self, event: &Event) -> Result<(), FrameError> {
    match event {
      Event::BrowserFrame(frame) => self.write_with_bytes(event, &frame.jpeg).await,
      Event::Reply { reply, .. } => match file_bytes(reply) {
        Some(file) => self.write_with_bytes(event, &file.bytes).await,
        None => self.write(event).await,
      },
      _ => self.write(event).await,
    }
  }
}

/// The bytes `reply` carries in a binary frame: a run's file read.
fn file_bytes(reply: &Reply) -> Option<&FileBytes> {
  match reply {
    Reply::RunFiles(RunFilesReply::Read(RunFileRead::Bytes(file))) => Some(file),
    _ => None,
  }
}

/// [`file_bytes`], to fill in.
fn file_bytes_mut(event: &mut Event) -> Option<&mut FileBytes> {
  match event {
    Event::Reply {
      reply: Reply::RunFiles(RunFilesReply::Read(RunFileRead::Bytes(file))),
      ..
    } => Some(file),
    _ => None,
  }
}

impl<R: AsyncRead + Unpin> FrameReader<R> {
  /// The next event, a screencast frame with its JPEG and a run's file read with its bytes; `None` when the peer
  /// closed the connection between frames. Either in a JSON frame, or bytes after any other event, is malformed.
  pub async fn read_event(&mut self) -> Result<Option<Event>, FrameError> {
    let Some(Frame { mut message, bytes }) = self.read_frame::<Event>().await? else {
      return Ok(None);
    };
    if let Some(file) = file_bytes_mut(&mut message) {
      file.bytes = bytes.ok_or(FrameError::Malformed("a run's file without its bytes"))?;
      return Ok(Some(message));
    }
    match (&mut message, bytes) {
      (Event::BrowserFrame(frame), Some(jpeg)) => frame.jpeg = jpeg,
      (Event::BrowserFrame(_), None) => return Err(FrameError::Malformed("a screencast frame without its bytes")),
      (_, Some(_)) => return Err(FrameError::Malformed("bytes after an event that carries none")),
      (_, None) => {}
    }
    Ok(Some(message))
  }
}
