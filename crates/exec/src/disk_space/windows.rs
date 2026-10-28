//! Free space on Windows: `GetDiskFreeSpaceExW`, the bytes available to this process's user on the disk of a folder.
//! std reads no free space, and windows-sys has no safe wrapper, so this module calls it directly.

#![allow(unsafe_code)]

use std::{io, os::windows::ffi::OsStrExt, path::Path, ptr};

use orchester_core::workspaces::DiskUsage;
use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

pub(super) fn usage(folder: &Path) -> io::Result<DiskUsage> {
  let wide: Vec<u16> = folder.as_os_str().encode_wide().chain(Some(0)).collect();
  let (mut available, mut total) = (0_u64, 0_u64);
  // SAFETY: `wide` is a NUL-terminated UTF-16 path that outlives the call; both out pointers point to live `u64`s
  // of this frame, and the total free bytes are not asked for (null).
  let succeeded = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &raw mut available, &raw mut total, ptr::null_mut()) };
  if succeeded == 0 {
    return Err(io::Error::last_os_error());
  }
  Ok(DiskUsage {
    free: available,
    total,
  })
}
