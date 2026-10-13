use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{
  IpcError,
  frame::{FrameReader, FrameWriter},
};

/// Version of the command and event messages. Bump it on any change a peer built from an
/// older revision could misread; desktop and runner ship together, so a mismatch means a
/// stale runner binary and is reported instead of guessed around.
pub const PROTOCOL_VERSION: u32 = 25;

/// The first frame each side sends. Its shape never changes and unknown fields are ignored,
/// so any two versions can at least tell each other which version they speak.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Handshake {
  protocol_version: u32,
}

impl Handshake {
  pub const fn new(protocol_version: u32) -> Self {
    Self { protocol_version }
  }

  pub const fn protocol_version(self) -> u32 {
    self.protocol_version
  }
}

/// Sends this side's version, then reads the peer's. Both sides call this; each writes
/// before reading, so each learns the other's version even when they differ.
pub async fn handshake<R, W>(reader: &mut FrameReader<R>, writer: &mut FrameWriter<W>) -> Result<(), IpcError>
where
  R: AsyncRead + Unpin,
  W: AsyncWrite + Unpin,
{
  writer.write(&Handshake::new(PROTOCOL_VERSION)).await?;
  let remote: Handshake = reader.read().await?.ok_or(IpcError::ClosedDuringHandshake)?;
  if remote.protocol_version != PROTOCOL_VERSION {
    return Err(IpcError::VersionMismatch {
      local: PROTOCOL_VERSION,
      remote: remote.protocol_version,
    });
  }
  Ok(())
}
