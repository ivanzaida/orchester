#![allow(unsafe_code)]
//! The Windows side of the search path: the `Path` Windows keeps for the machine and for the user, searched after the
//! inherited `PATH`. Explorer gives the programs it starts both, but a program started by another one inherits that
//! one's `PATH`, which can be older or shorter: Orchester started by its installer, opened from a browser that was
//! started before Git was installed, has no Git folder on its `PATH`. As the login shell's `PATH` on macOS, it is
//! added only in a process that asks for it as it starts ([`start_reading`]), and then read again for each search and
//! each program started. Windows has no safe wrapper for reading a registry value (`RegGetValueW`); each call below is
//! one FFI call on buffers this module owns.

use std::{
  env,
  ffi::OsString,
  os::windows::ffi::OsStringExt,
  path::PathBuf,
  process::Command as Process,
  ptr,
  sync::atomic::{AtomicBool, Ordering},
};

use orchester_core::command::Command;
use windows_sys::Win32::{
  Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA},
  System::Registry::{HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW},
};

use super::SearchPath;

/// Where Windows keeps the machine's environment variables.
const MACHINE: &str = r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment";
/// Where Windows keeps the user's environment variables.
const USER: &str = "Environment";
const PATH: &str = "Path";
/// Room for a `Path` of up to 4096 characters; a longer one is read again with the size Windows names.
const FIRST_ROOM: usize = 4096;
/// Reads of one value: the value can grow between asking its size and reading it.
const READS: usize = 3;

/// Whether this process adds the `Path` Windows keeps to its search path ([`start_reading`]).
static READING: AtomicBool = AtomicBool::new(false);

/// Adds the `Path` Windows keeps to the search path of this process and of the programs it starts. Called once, as the
/// app starts; a process that does not call it (the runner, which inherits the app's search path) searches its `PATH`
/// alone.
pub(super) fn start_reading() {
  READING.store(true, Ordering::Release);
}

/// The folders [`registry_folders`] reads, in a process that adds them ([`start_reading`]); none in another.
pub(super) fn added_folders() -> Vec<PathBuf> {
  if READING.load(Ordering::Acquire) {
    registry_folders()
  } else {
    Vec::new()
  }
}

/// The folders of the machine's `Path`, then the user's, in the order Windows gives them to a program Explorer starts,
/// with variables such as `%USERPROFILE%` expanded. A value that is not there or cannot be read adds none.
pub fn registry_folders() -> Vec<PathBuf> {
  [(HKEY_LOCAL_MACHINE, MACHINE), (HKEY_CURRENT_USER, USER)]
    .into_iter()
    .filter_map(|(hive, key)| text_value(hive, key, PATH))
    .flat_map(|path| env::split_paths(&path).collect::<Vec<_>>())
    .filter(|folder| !folder.as_os_str().is_empty())
    .collect()
}

/// Gives the process of `command` the search path as its `PATH`: the `PATH` the command sets or inherits, then the
/// machine's and the user's `Path`. Windows looks a program without a folder up on this `PATH` too. Nothing in a
/// process that does not add the `Path` ([`start_reading`]): its programs get the `PATH` as it is.
pub(crate) fn apply(process: &mut Process, command: &Command) {
  if !READING.load(Ordering::Acquire) {
    return;
  }
  let search = SearchPath::merge(super::command_path(command).as_deref(), &[], &[]).with_registry(&registry_folders());
  process.env("PATH", search.joined());
}

/// The text value `value` of `key` in `hive`, expanded when it holds variables. `None` when it is not there; the log
/// says when it cannot be read, and the search path goes without it.
fn text_value(hive: HKEY, key: &str, value: &str) -> Option<OsString> {
  let (wide_key, wide_value) = (wide(key), wide(value));
  let mut room = FIRST_ROOM;
  for _ in 0..READS {
    let mut data = vec![0_u16; room];
    let mut size = u32::try_from(data.len() * size_of::<u16>()).ok()?;
    // SAFETY: `wide_key` and `wide_value` are NUL-terminated and `data` is writable for `size` bytes.
    // `RRF_RT_REG_SZ` takes a `REG_EXPAND_SZ` too and expands it.
    let status = unsafe {
      RegGetValueW(
        hive,
        wide_key.as_ptr(),
        wide_value.as_ptr(),
        RRF_RT_REG_SZ,
        ptr::null_mut(),
        data.as_mut_ptr().cast(),
        &raw mut size,
      )
    };
    match status {
      0 => {
        let end = data.iter().position(|&unit| unit == 0).unwrap_or(data.len());
        return Some(OsString::from_wide(&data[..end]));
      }
      ERROR_MORE_DATA => room = usize::try_from(size).ok()?.div_ceil(size_of::<u16>()),
      ERROR_FILE_NOT_FOUND => return None,
      error => {
        tracing::warn!(
          error,
          key,
          "cannot read the Path Windows keeps; programs are searched without it"
        );
        return None;
      }
    }
  }
  tracing::warn!(
    key,
    "the Path Windows keeps kept growing while read; programs are searched without it"
  );
  None
}

fn wide(text: &str) -> Vec<u16> {
  text.encode_utf16().chain([0]).collect()
}
