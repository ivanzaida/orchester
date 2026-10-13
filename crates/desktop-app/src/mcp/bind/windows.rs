//! A listener that holds its port with `SO_EXCLUSIVEADDRUSE`. The option has to be set between creating the socket and
//! binding it, which std's `TcpListener::bind` does in one call, and neither std nor socket2 sets it; so the socket is
//! made with socket2 and the option set through WinSock's `setsockopt`, an FFI call without a safe wrapper, which
//! this module allows `unsafe` for (AGENTS.md rule 9).

#![allow(unsafe_code)]

use std::{
  io,
  net::{SocketAddr, TcpListener},
  os::windows::io::AsRawSocket,
};

use socket2::{Domain, Protocol, Socket, Type};
use windows_sys::Win32::Networking::WinSock::{SO_EXCLUSIVEADDRUSE, SOCKET, SOL_SOCKET, setsockopt};

/// Connections waiting to be accepted; local clients need a few.
const BACKLOG: i32 = 128;

pub(super) fn listen(address: SocketAddr) -> io::Result<TcpListener> {
  let socket = Socket::new(Domain::for_address(address), Type::STREAM, Some(Protocol::TCP))?;
  let raw = SOCKET::try_from(socket.as_raw_socket()).map_err(io::Error::other)?;
  let on: i32 = 1;
  let length = i32::try_from(size_of::<i32>()).expect("the size of an i32 fits an i32");
  // SAFETY: `raw` is the handle of `socket`, which is open and owned here for the whole call; `optval` points at
  // `on`, which lives across the call, and `optlen` is its size, so setsockopt reads exactly the 4 bytes of `on`.
  let result = unsafe {
    setsockopt(
      raw,
      SOL_SOCKET,
      SO_EXCLUSIVEADDRUSE,
      (&raw const on).cast::<u8>(),
      length,
    )
  };
  if result != 0 {
    // WinSock's last error is the thread's last error, which `last_os_error` reads.
    return Err(io::Error::last_os_error());
  }
  socket.bind(&address.into())?;
  socket.listen(BACKLOG)?;
  Ok(socket.into())
}
