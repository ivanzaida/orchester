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

/// Whether binding failed because another socket holds the port. On Windows a port held with
/// `SO_EXCLUSIVEADDRUSE`, or in a range the system keeps for itself, answers "access denied".
pub fn taken(error: &io::Error) -> bool {
  error.kind() == io::ErrorKind::AddrInUse || (cfg!(windows) && error.kind() == io::ErrorKind::PermissionDenied)
}
