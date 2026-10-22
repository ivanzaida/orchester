#![allow(unsafe_code)]
//! The Recycle Bin through the shell itself. Windows has no safe wrapper for the shell's recycling move
//! (`SHFileOperationW`), the bin's item count (`SHQueryRecycleBinW`), the volume a path is on, or the registry values
//! that say whether a volume's bin keeps what it is given; each call below is one FFI call on buffers this module
//! owns. The move runs with `FOF_ALLOWUNDO | FOF_NO_UI`: it never asks in a dialog, and what the bin can't take the
//! shell would delete for good, so [`bin_of`] refuses those volumes and items before, and [`recycle`] checks the bin
//! received the item after.

use std::{
  fs::{self, OpenOptions},
  io, iter,
  os::windows::{ffi::OsStrExt, fs::OpenOptionsExt},
  path::Path,
  ptr,
};

use windows_sys::Win32::{
  Foundation::RPC_E_CHANGED_MODE,
  Storage::FileSystem::{DELETE, GetDriveTypeW, GetVolumeNameForVolumeMountPointW, GetVolumePathNameW},
  System::{
    Com::{COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize},
    Registry::{HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RegGetValueW},
  },
  UI::Shell::{
    FO_DELETE, FOF_ALLOWUNDO, FOF_NO_UI, SHFILEOPSTRUCTW, SHFileOperationW, SHQUERYRBINFO, SHQueryRecycleBinW,
  },
};

use super::{Bin, TrashRefusal};

/// `GetDriveTypeW`'s answer for a fixed disk, the only kind whose volumes have a Recycle Bin of their own.
const DRIVE_FIXED: u32 = 3;
/// Where Explorer keeps each volume's Recycle Bin settings, under the volume's GUID.
const BIT_BUCKET: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\BitBucket\Volume";
/// The policy that turns every Recycle Bin off (`NoRecycleFiles`).
const POLICIES: &str = r"Software\Microsoft\Windows\CurrentVersion\Policies\Explorer";
/// Room for a volume root or a volume GUID path.
const NAME_ROOM: usize = 1024;

pub fn ensure_movable(path: &Path) -> io::Result<()> {
  if !fs::symlink_metadata(path)?.is_file() {
    return Ok(());
  }
  // `DELETE` access, which moving needs, shared with no one: refused while any other program holds the file.
  OpenOptions::new()
    .access_mode(DELETE)
    .share_mode(0)
    .open(path)
    .map(drop)
}

pub fn bin_of(path: &Path) -> io::Result<Bin> {
  let root = match volume_root(path) {
    Ok(root) => root,
    Err(error) if TrashRefusal::of(&error).is_some() => return Ok(Bin::Missing),
    Err(error) => return Err(error),
  };
  // SAFETY: `root` is a NUL-terminated volume root this module built.
  if unsafe { GetDriveTypeW(root.as_ptr()) } != DRIVE_FIXED {
    return Ok(Bin::Missing);
  }
  // A substituted drive or a folder mounted from the network has no volume GUID, nor a bin of its own.
  let Some(volume) = volume_guid(&root) else {
    return Ok(Bin::Missing);
  };
  let policy = [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE]
    .into_iter()
    .any(|hive| dword(hive, POLICIES, "NoRecycleFiles") == Some(1));
  let key = format!(r"{BIT_BUCKET}\{volume}");
  if policy || dword(HKEY_CURRENT_USER, &key, "NukeOnDelete") == Some(1) || count(&root).is_err() {
    return Ok(Bin::Missing);
  }
  Ok(Bin::Keeps {
    capacity: dword(HKEY_CURRENT_USER, &key, "MaxCapacity").map(|megabytes| u64::from(megabytes) * 1024 * 1024),
  })
}

pub fn recycle(path: &Path) -> io::Result<()> {
  let root = volume_root(path)?;
  let before = count(&root)?;
  // The shell takes a list of paths, each NUL-terminated, the list ended by one more NUL.
  let from = terminated(plain(path)?, 2);
  let _apartment = Apartment::enter()?;
  let mut operation = SHFILEOPSTRUCTW {
    wFunc: FO_DELETE,
    pFrom: from.as_ptr(),
    pTo: ptr::null(),
    fFlags: u16::try_from(FOF_ALLOWUNDO | FOF_NO_UI).expect("the shell's operation flags fit in 16 bits"),
    ..Default::default()
  };
  // SAFETY: `operation` is fully initialized; `pFrom` points into `from`, a double-NUL-terminated list that outlives
  // the call; no window, no destination, no name mappings are asked for.
  let code = unsafe { SHFileOperationW(&raw mut operation) };
  if code != 0 {
    return Err(shell_error(code));
  }
  if operation.fAnyOperationsAborted != 0 {
    return Err(io::Error::other("the system stopped before moving it"));
  }
  if fs::symlink_metadata(path).is_ok() {
    return Err(io::Error::other("it is still in its place after the move"));
  }
  if count(&root)? <= before {
    return Err(TrashRefusal::NotKept.into());
  }
  Ok(())
}

/// COM on this thread for the shell's move, let go of when this is dropped if this call took it.
struct Apartment(bool);

impl Apartment {
  fn enter() -> io::Result<Self> {
    let model = u32::try_from(COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).expect("COM's flags are positive");
    // SAFETY: the reserved pointer is null, as required; a success is balanced by `CoUninitialize` in `drop`.
    let result = unsafe { CoInitializeEx(ptr::null(), model) };
    match result {
      result if result >= 0 => Ok(Self(true)),
      // The thread is in COM already, in another model: the shell works there too, and that call keeps it.
      RPC_E_CHANGED_MODE => Ok(Self(false)),
      result => Err(io::Error::from_raw_os_error(result)),
    }
  }
}

impl Drop for Apartment {
  fn drop(&mut self) {
    if self.0 {
      // SAFETY: balances this thread's successful `CoInitializeEx` in `enter`.
      unsafe { CoUninitialize() };
    }
  }
}

/// What the shell's move answered: its own codes (`DE_*`) are not system errors and are named by number.
fn shell_error(code: i32) -> io::Error {
  let shell_own = matches!(code, 0x71..=0x88 | 0xB7 | 0x402 | 0x10000 | 0x10074);
  if shell_own {
    io::Error::other(format!("the system refused to move it (shell error {code:#x})"))
  } else {
    io::Error::from_raw_os_error(code)
  }
}

/// The items in the Recycle Bin of the volume `root` (NUL-terminated).
fn count(root: &[u16]) -> io::Result<i64> {
  let mut info = SHQUERYRBINFO {
    cbSize: u32::try_from(size_of::<SHQUERYRBINFO>()).expect("the structure's size fits in 32 bits"),
    ..Default::default()
  };
  // SAFETY: `root` is NUL-terminated and `info` is a writable structure with its size set, as the call requires.
  let result = unsafe { SHQueryRecycleBinW(root.as_ptr(), &raw mut info) };
  if result < 0 {
    return Err(io::Error::from_raw_os_error(result));
  }
  Ok(info.i64NumItems)
}

/// The root of the volume `path` is on, e.g. `H:\`, NUL-terminated.
fn volume_root(path: &Path) -> io::Result<Vec<u16>> {
  let name = terminated(plain(path)?, 1);
  let mut root = vec![0; name.len().max(NAME_ROOM)];
  let room = u32::try_from(root.len()).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
  // SAFETY: `name` is NUL-terminated and `root` is writable for `room` characters.
  if unsafe { GetVolumePathNameW(name.as_ptr(), root.as_mut_ptr(), room) } == 0 {
    return Err(io::Error::last_os_error());
  }
  Ok(until_nul(root))
}

/// The volume's GUID in braces, as Explorer names its bin settings; `None` for a volume without one.
fn volume_guid(root: &[u16]) -> Option<String> {
  let mut name = vec![0; NAME_ROOM];
  let room = u32::try_from(name.len()).ok()?;
  // SAFETY: `root` is a NUL-terminated volume root and `name` is writable for `room` characters.
  if unsafe { GetVolumeNameForVolumeMountPointW(root.as_ptr(), name.as_mut_ptr(), room) } == 0 {
    return None;
  }
  let name = String::from_utf16(&until_nul(name)[..]).ok()?;
  let start = name.find('{')?;
  let end = name.find('}')?;
  (start < end).then(|| name[start..=end].to_owned())
}

/// The `DWORD` registry value `value` of `key` in `hive`; `None` when it is not there.
fn dword(hive: HKEY, key: &str, value: &str) -> Option<u32> {
  let (key, value) = (wide(key), wide(value));
  let mut data = 0_u32;
  let mut size = u32::try_from(size_of::<u32>()).ok()?;
  // SAFETY: `key` and `value` are NUL-terminated, `data` is a writable DWORD and `size` says its size.
  let status = unsafe {
    RegGetValueW(
      hive,
      key.as_ptr(),
      value.as_ptr(),
      RRF_RT_REG_DWORD,
      ptr::null_mut(),
      (&raw mut data).cast(),
      &raw mut size,
    )
  };
  (status == 0).then_some(data)
}

/// `path` as the shell takes it: absolute, without the verbatim prefix `\\?\`, not terminated. A verbatim network path
/// has no Recycle Bin.
fn plain(path: &Path) -> io::Result<Vec<u16>> {
  if !path.is_absolute() {
    return Err(io::Error::from(io::ErrorKind::InvalidInput));
  }
  let full: Vec<u16> = path.as_os_str().encode_wide().collect();
  let verbatim = wide_text(r"\\?\");
  let Some(rest) = full.strip_prefix(&verbatim[..]) else {
    return Ok(full);
  };
  if rest.starts_with(&wide_text(r"UNC\")[..]) {
    return Err(TrashRefusal::NoBin.into());
  }
  Ok(rest.to_vec())
}

fn wide(text: &str) -> Vec<u16> {
  terminated(wide_text(text), 1)
}

fn wide_text(text: &str) -> Vec<u16> {
  text.encode_utf16().collect()
}

fn terminated(mut text: Vec<u16>, nuls: usize) -> Vec<u16> {
  text.extend(iter::repeat_n(0, nuls));
  text
}

/// `buffer` up to its first NUL, NUL-terminated.
fn until_nul(mut buffer: Vec<u16>) -> Vec<u16> {
  let end = buffer.iter().position(|unit| *unit == 0).unwrap_or(buffer.len());
  buffer.truncate(end);
  terminated(buffer, 1)
}
