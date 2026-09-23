//! Asks Windows which process serves a named pipe, so the desktop only talks to the runner it
//! started: another process of the same user could otherwise create the pipe name first.
//!
//! This is FFI with no safe wrapper available, so `unsafe` is allowed here and only here.
#![allow(unsafe_code)]

use std::{io, os::windows::io::AsRawHandle};

use windows_sys::Win32::System::Pipes::GetNamedPipeServerProcessId;

pub(crate) fn server_process_id(pipe: &impl AsRawHandle) -> io::Result<u32> {
  let mut process_id: u32 = 0;
  // SAFETY: `pipe` is an open client handle of a named pipe, borrowed for the whole call, and
  // `process_id` is a live, writable `u32` the function only writes to.
  let succeeded = unsafe { GetNamedPipeServerProcessId(pipe.as_raw_handle(), &mut process_id) };
  if succeeded == 0 {
    return Err(io::Error::last_os_error());
  }
  Ok(process_id)
}
