//! Which process listens on a TCP port of the loopback interface: what the `cdp` UI driver asks before it attaches to
//! an app's remote debugging port, so that it attaches only to a process of the service's own tree
//! (`docs/architecture/agent-testing.md`, "UI drivers"). Windows reads the system's TCP table; elsewhere it is not
//! supported, as the driver is not (WKWebView on macOS has no debugging port).

#[cfg(windows)]
mod windows;

use std::io;

/// The pid of the process listening on `port` for IPv4 connections to `127.0.0.1`: bound to the loopback address or
/// to every address. `None` when none does.
#[cfg(windows)]
pub fn loopback_owner(port: u16) -> io::Result<Option<u32>> {
  windows::loopback_owner(port)
}

/// Not supported off Windows.
#[cfg(not(windows))]
pub fn loopback_owner(port: u16) -> io::Result<Option<u32>> {
  Err(io::Error::new(
    io::ErrorKind::Unsupported,
    format!("the process listening on port {port} cannot be told on this platform"),
  ))
}
