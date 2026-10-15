#![allow(unsafe_code)]
//! A private NTFS junction probe. No real private paths or global input are used.
use std::{
  fs::OpenOptions,
  io,
  os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
  path::Path,
};

use windows_sys::Win32::{
  Storage::FileSystem::{
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
  },
  System::{IO::DeviceIoControl, Ioctl::FSCTL_SET_REPARSE_POINT},
};

pub fn set_junction(directory: &Path, target: &Path) -> io::Result<()> {
  let file = OpenOptions::new()
    .read(true)
    .write(true)
    .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
    .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
    .open(directory)?;
  let target = target.canonicalize()?;
  let name = target
    .to_str()
    .ok_or(io::ErrorKind::InvalidInput)?
    .strip_prefix(r"\\?\")
    .unwrap_or(target.to_str().ok_or(io::ErrorKind::InvalidInput)?);
  let substitute: Vec<u16> = format!(r"\??\{name}").encode_utf16().collect();
  let length = u16::try_from(substitute.len() * 2).map_err(|_| io::ErrorKind::InvalidInput)?;
  let mut buffer = Vec::new();
  buffer.extend_from_slice(&0xa0000003u32.to_le_bytes()); // IO_REPARSE_TAG_MOUNT_POINT
  buffer.extend_from_slice(&(length + 12).to_le_bytes());
  buffer.extend_from_slice(&0u16.to_le_bytes());
  for value in [0, length, length + 2, 0] {
    buffer.extend_from_slice(&value.to_le_bytes());
  }
  for value in substitute.into_iter().chain([0, 0]) {
    buffer.extend_from_slice(&value.to_le_bytes());
  }
  let mut returned = 0;
  // SAFETY: the live directory handle receives a complete mount-point buffer with checked UTF-16 lengths.
  let success = unsafe {
    DeviceIoControl(
      file.as_raw_handle(),
      FSCTL_SET_REPARSE_POINT,
      buffer.as_ptr().cast(),
      u32::try_from(buffer.len()).map_err(|_| io::ErrorKind::InvalidInput)?,
      std::ptr::null_mut(),
      0,
      &raw mut returned,
      std::ptr::null_mut(),
    )
  };
  if success == 0 {
    return Err(io::Error::last_os_error());
  }
  Ok(())
}
