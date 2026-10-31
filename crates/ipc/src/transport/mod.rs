//! The local channel between the desktop and one project's runner. Both processes derive the
//! endpoint from the canonical project root, so they agree on it without passing it around,
//! and different projects never share one. Each runner accepts exactly one connection.
//!
//! - Windows: a named pipe ([`windows`]).
//! - macOS: a Unix domain socket in a per-user folder ([`unix`]).
//!
//! Both give the same [`Endpoint`] API and byte streams, so framing, handshake and messages are
//! identical on every platform.

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use self::unix::{ClientStream, Endpoint, ServerStream, server_process_id};
#[cfg(windows)]
pub use self::windows::{ClientStream, Endpoint, ServerStream};
/// The process serving the endpoint the desktop connected to.
#[cfg(windows)]
pub use crate::pipe_owner::server_process_id;

/// 64-bit FNV-1a. Stable across builds, unlike `std`'s `DefaultHasher`, which matters because
/// both processes must compute the same name. The app's `orchester://` link pipe names its data folder by it too.
pub fn fnv1a(bytes: &[u8]) -> u64 {
  const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
  const PRIME: u64 = 0x0000_0100_0000_01b3;
  bytes
    .iter()
    .fold(OFFSET, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(PRIME))
}
