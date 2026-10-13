//! The server's listening socket, which no other socket may share: with a fixed port, a program that could bind it
//! beside the server would take some of its connections while Settings says it listens.
//!
//! On Windows a socket another program opens with `SO_REUSEADDR` can bind a port that is already listening unless the
//! listener holds it with `SO_EXCLUSIVEADDRUSE`, which std does not set (`windows`). On macOS a socket can share a
//! listening TCP port only when every socket on it set `SO_REUSEPORT`, which std does not; `SO_REUSEADDR` alone only
//! lets another socket bind the wildcard address beside the server's `127.0.0.1`, and connections to `127.0.0.1` keep
//! reaching the more specific one, the server's. So std's own bind is the macOS listener.

#[cfg(windows)]
mod windows;

use std::{
  io,
  net::{SocketAddr, TcpListener},
};

use super::status::Taken;

/// A listener on `address` that no other socket shares.
#[cfg(windows)]
pub fn listen(address: SocketAddr) -> io::Result<TcpListener> {
  windows::listen(address)
}

/// A listener on `address` that no other socket shares.
#[cfg(not(windows))]
pub fn listen(address: SocketAddr) -> io::Result<TcpListener> {
  TcpListener::bind(address)
}

impl Taken {
  /// How binding the port failed, when the port is kept from the server: another program listens on it, or (on
  /// Windows, "access denied") the system reserves it or another program holds it with `SO_EXCLUSIVEADDRUSE`.
  pub fn of_bind_error(error: &io::Error) -> Option<Self> {
    match error.kind() {
      io::ErrorKind::AddrInUse => Some(Self::InUse),
      io::ErrorKind::PermissionDenied if cfg!(windows) => Some(Self::Refused),
      _ => None,
    }
  }
}
