#![allow(unsafe_code)]
//! Stable std has no safe wrapper for a file's NTFS change time or its file index; both are read through a handle
//! opened for attributes only, so no content is read and nothing else is kept from sharing the file.

use std::{
  fs::OpenOptions,
  mem,
  os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
  path::Path,
};

use windows_sys::Win32::Storage::FileSystem::{
  BY_HANDLE_FILE_INFORMATION, FILE_BASIC_INFO, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
  FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FileBasicInfo,
  GetFileInformationByHandle, GetFileInformationByHandleEx,
};

use super::Stamp;

/// 100-nanosecond intervals from 1601-01-01 (a `FILETIME`'s origin) to the Unix epoch.
const EPOCH_INTERVALS: i64 = 116_444_736_000_000_000;

/// The stamp of the entry at `path`, its reparse point if it is one; none when it cannot be read, or when the file
/// system keeps no change time (FAT), which only means it is hashed.
pub(super) fn stamp(path: &Path) -> Option<Stamp> {
  let file = OpenOptions::new()
    .access_mode(FILE_READ_ATTRIBUTES)
    .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
    .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
    .open(path)
    .ok()?;
  let mut basic = FILE_BASIC_INFO::default();
  // SAFETY: file owns a live handle and basic is a writable FILE_BASIC_INFO of exactly the size passed.
  let read = unsafe {
    GetFileInformationByHandleEx(
      file.as_raw_handle(),
      FileBasicInfo,
      (&raw mut basic).cast(),
      mem::size_of::<FILE_BASIC_INFO>() as u32,
    )
  };
  if read == 0 || basic.ChangeTime == 0 {
    return None;
  }
  let mut info = BY_HANDLE_FILE_INFORMATION::default();
  // SAFETY: file owns a live handle and info is a writable BY_HANDLE_FILE_INFORMATION.
  if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &raw mut info) } == 0 {
    return None;
  }
  Some(Stamp {
    size: u64::from(info.nFileSizeHigh) << 32 | u64::from(info.nFileSizeLow),
    modified: time(basic.LastWriteTime),
    changed: time(basic.ChangeTime),
    identity: (
      u64::from(info.dwVolumeSerialNumber),
      u64::from(info.nFileIndexHigh) << 32 | u64::from(info.nFileIndexLow),
    ),
    kind: basic.FileAttributes,
  })
}

fn time(intervals: i64) -> i128 {
  (i128::from(intervals) - i128::from(EPOCH_INTERVALS)) * 100
}
