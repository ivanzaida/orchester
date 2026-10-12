#![allow(unsafe_code)]
//! FFI without safe std wrappers: open each canonical component relative to the held parent (NtOpenFile),
//! query the opened type, and enumerate the held directory itself (GetFileInformationByHandleEx).
//! No absolute descendant lookup or byte access precedes the opened-object type check.
use std::{
  ffi::{OsStr, OsString},
  fs::{File, Metadata, OpenOptions},
  io, mem,
  os::windows::{
    ffi::{OsStrExt, OsStringExt},
    fs::{MetadataExt, OpenOptionsExt},
    io::{AsRawHandle, FromRawHandle},
  },
  path::Path,
  ptr,
};

use windows_sys::{
  Wdk::{
    Foundation::OBJECT_ATTRIBUTES,
    Storage::FileSystem::{FILE_OPEN_REPARSE_POINT, FILE_SYNCHRONOUS_IO_NONALERT, NtOpenFile},
  },
  Win32::{
    Foundation::{ERROR_NO_MORE_FILES, RtlNtStatusToDosError, UNICODE_STRING},
    Storage::FileSystem::{
      FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_GENERIC_READ,
      FILE_ID_BOTH_DIR_INFO, FILE_SHARE_READ, FILE_TYPE_DISK, FileIdBothDirectoryInfo, FileIdBothDirectoryRestartInfo,
      GetFileInformationByHandleEx, GetFileType,
    },
    System::IO::IO_STATUS_BLOCK,
  },
};

use super::changed;

pub(super) fn root_part(path: &Path, parent: Option<&File>) -> io::Result<File> {
  if let Some(parent) = parent {
    return child(parent, path.file_name().ok_or_else(changed)?, path);
  }
  // Only the volume root is opened absolutely; all further components use that held object.
  OpenOptions::new()
    .read(true)
    .share_mode(FILE_SHARE_READ)
    .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
    .open(path)
}
pub(super) fn child(parent: &File, name: &OsStr, _: &Path) -> io::Result<File> {
  let mut wide = name.encode_wide().collect::<Vec<_>>();
  let length = wide
    .len()
    .checked_mul(2)
    .and_then(|n| u16::try_from(n).ok())
    .ok_or_else(changed)?;
  let mut name = UNICODE_STRING {
    Length: length,
    MaximumLength: length,
    Buffer: wide.as_mut_ptr(),
  };
  let attributes = OBJECT_ATTRIBUTES {
    Length: mem::size_of::<OBJECT_ATTRIBUTES>() as u32,
    RootDirectory: parent.as_raw_handle(),
    ObjectName: &raw mut name,
    Attributes: 0x40,
    SecurityDescriptor: ptr::null_mut(),
    SecurityQualityOfService: ptr::null_mut(),
  };
  let mut handle = ptr::null_mut();
  // SAFETY: zeroed IO_STATUS_BLOCK is writable plain output storage.
  let mut status: IO_STATUS_BLOCK = unsafe { mem::zeroed() };
  // SAFETY: parent is held, name contains one checked component; all input/output storage lives through the call.
  let result = unsafe {
    NtOpenFile(
      &raw mut handle,
      FILE_GENERIC_READ,
      &attributes,
      &raw mut status,
      FILE_SHARE_READ,
      FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
    )
  };
  if result < 0 {
    // SAFETY: result is the NTSTATUS returned by NtOpenFile, converted without dereferencing anything.
    return Err(io::Error::from_raw_os_error(
      unsafe { RtlNtStatusToDosError(result) } as i32
    ));
  }
  // SAFETY: successful NtOpenFile transferred one handle; File now owns and closes it exactly once.
  Ok(unsafe { File::from_raw_handle(handle) })
}
pub(super) fn metadata(file: &File) -> io::Result<Metadata> {
  // SAFETY: file owns this live handle throughout the non-mutating query.
  let kind = unsafe { GetFileType(file.as_raw_handle()) };
  let metadata = file.metadata()?;
  if kind != FILE_TYPE_DISK || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
    return Err(changed());
  }
  Ok(metadata)
}
pub(super) fn directory(file: &File) -> io::Result<()> {
  if metadata(file)?.is_dir() {
    Ok(())
  } else {
    Err(changed())
  }
}
pub(super) fn names(file: &File, _: &Path, maximum: usize) -> io::Result<Vec<OsString>> {
  // u64 supplies the directory-info struct's alignment; storage stays initialized and bounded.
  let mut buffer = vec![0_u64; 512];
  let mut names = Vec::new();
  let mut class = FileIdBothDirectoryRestartInfo;
  loop {
    // SAFETY: file is a checked held directory, and the aligned buffer is writable for its full byte length.
    let result = unsafe {
      GetFileInformationByHandleEx(
        file.as_raw_handle(),
        class,
        buffer.as_mut_ptr().cast(),
        (buffer.len() * mem::size_of::<u64>()) as u32,
      )
    };
    if result == 0 {
      let error = io::Error::last_os_error();
      if error.raw_os_error() == Some(ERROR_NO_MORE_FILES as i32) {
        return Ok(names);
      }
      return Err(error);
    }
    decode(&buffer, &mut names, maximum)?;
    class = FileIdBothDirectoryInfo;
  }
}
fn decode(buffer: &[u64], names: &mut Vec<OsString>, maximum: usize) -> io::Result<()> {
  let capacity = mem::size_of_val(buffer);
  let header = mem::offset_of!(FILE_ID_BOTH_DIR_INFO, FileName);
  let mut offset = 0;
  loop {
    if offset + mem::size_of::<FILE_ID_BOTH_DIR_INFO>() > capacity {
      return Err(changed());
    }
    // SAFETY: checked buffer bounds cover a whole plain struct; read_unaligned also handles the next entry offset.
    let entry =
      unsafe { ptr::read_unaligned(buffer.as_ptr().cast::<u8>().add(offset).cast::<FILE_ID_BOTH_DIR_INFO>()) };
    let length = entry.FileNameLength as usize;
    let start = offset + header;
    if !length.is_multiple_of(2) || !start.is_multiple_of(2) || length > capacity - start {
      return Err(changed());
    }
    // SAFETY: checked initialized buffer bounds and u16 alignment cover exactly FileNameLength bytes.
    let wide = unsafe { std::slice::from_raw_parts(buffer.as_ptr().cast::<u8>().add(start).cast::<u16>(), length / 2) };
    let name = OsString::from_wide(wide);
    if name != "." && name != ".." {
      if names.len() == maximum {
        return Err(io::Error::new(io::ErrorKind::FileTooLarge, "discovery entry limit"));
      }
      names.push(name);
    }
    let next = entry.NextEntryOffset as usize;
    if next == 0 {
      return Ok(());
    }
    if next < header + length || next > capacity - offset {
      return Err(changed());
    }
    offset += next;
  }
}
