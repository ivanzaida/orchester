#![allow(unsafe_code)]
//! Whether a drive letter names a network share (a mapped drive, or one `subst` points at a share). The standard
//! library has no call that tells, and resolving a path on such a drive contacts the server it names, so this module
//! asks Windows with `GetDriveTypeW` (FFI without a safe wrapper, AGENTS.md rule 9). The call reads the drive's local
//! mapping; it does not contact the server. Windows only: a Unix path has no drive letter.

use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;

/// `GetDriveTypeW`'s answer for a network drive.
const DRIVE_REMOTE: u32 = 4;

/// Whether the drive `letter` (`'Z'`) is mapped to a network share.
pub(crate) fn is_remote(letter: char) -> bool {
  let root: Vec<u16> = format!("{letter}:\\").encode_utf16().chain([0]).collect();
  // SAFETY: `root` is a NUL-terminated wide string that outlives the call, which only reads it.
  unsafe { GetDriveTypeW(root.as_ptr()) == DRIVE_REMOTE }
}
