#![allow(unsafe_code)]
//! The Toolhelp process snapshot has no std safe wrapper; the console test reads it to wait for the console hosts
//! of its own stand-ins before it stops them.

use std::{ffi::OsString, io, mem, os::windows::ffi::OsStringExt};

use windows_sys::Win32::{
  Foundation::{CloseHandle, ERROR_NO_MORE_FILES, INVALID_HANDLE_VALUE},
  System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
  },
};

/// The pids of the running processes named `name` whose parent is `parent`.
pub fn children_named(parent: u32, name: &str) -> io::Result<Vec<u32>> {
  // SAFETY: flags and PID are the documented process snapshot arguments.
  let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
  if snapshot == INVALID_HANDLE_VALUE {
    return Err(io::Error::last_os_error());
  }
  // SAFETY: PROCESSENTRY32W is a C record whose fields accept zero.
  let mut entry: PROCESSENTRY32W = unsafe { mem::zeroed() };
  entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as u32;
  let mut found = Vec::new();
  // SAFETY: the snapshot is live and entry has the required size.
  let mut more = unsafe { Process32FirstW(snapshot, &mut entry) };
  while more != 0 {
    let length = entry
      .szExeFile
      .iter()
      .position(|unit| *unit == 0)
      .unwrap_or(entry.szExeFile.len());
    let exe = OsString::from_wide(&entry.szExeFile[..length]);
    if entry.th32ParentProcessID == parent && exe.eq_ignore_ascii_case(name) {
      found.push(entry.th32ProcessID);
    }
    // SAFETY: same live snapshot and correctly sized writable entry.
    more = unsafe { Process32NextW(snapshot, &mut entry) };
  }
  let error = io::Error::last_os_error();
  // SAFETY: the snapshot handle is owned here and closed once.
  unsafe { CloseHandle(snapshot) };
  if error.raw_os_error() == Some(ERROR_NO_MORE_FILES as i32) {
    Ok(found)
  } else {
    Err(error)
  }
}
