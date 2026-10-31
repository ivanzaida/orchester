use std::{io, path::PathBuf};

use thiserror::Error;

/// A frame could not be read or written.
#[derive(Debug, Error)]
pub enum FrameError {
  #[error("connection error: {0}")]
  Io(#[from] io::Error),
  #[error("frame of {len} bytes exceeds the {max}-byte limit")]
  TooLarge { len: usize, max: u32 },
  #[error("the connection closed in the middle of a frame")]
  Truncated,
  /// A frame whose kind or layout does not fit what it carries (protocol 34's binary frames).
  #[error("malformed frame: {0}")]
  Malformed(&'static str),
  #[error("cannot decode frame: {0}")]
  Decode(serde_json::Error),
  #[error("cannot encode frame: {0}")]
  Encode(serde_json::Error),
}

/// A failure of the desktop-runner channel.
#[derive(Debug, Error)]
pub enum IpcError {
  #[error(transparent)]
  Frame(#[from] FrameError),
  #[error("protocol version mismatch: this side speaks version {local}, the other side speaks version {remote}")]
  VersionMismatch { local: u32, remote: u32 },
  #[error("the other side closed the connection during the handshake")]
  ClosedDuringHandshake,
  #[error("cannot resolve project root {path}: {source}")]
  ProjectRoot { path: PathBuf, source: io::Error },
  #[error("IPC endpoint {name} is already in use by another process")]
  EndpointInUse { name: String },
  #[error("IPC endpoint {name}: {source}")]
  Endpoint { name: String, source: io::Error },
  /// The per-user folder of the runner sockets cannot be created, or belongs to another user.
  #[cfg(unix)]
  #[error("IPC endpoint folder {path}: {source}")]
  EndpointFolder { path: PathBuf, source: io::Error },
}
