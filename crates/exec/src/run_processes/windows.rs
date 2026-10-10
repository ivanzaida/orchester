#![allow(unsafe_code)]
//! Win32 process handles and window messages have no std safe wrapper. Handles stay open through termination,
//! so PID reuse cannot redirect a validated stop to a different process.

use std::{ffi::OsString, io, mem, os::windows::ffi::OsStringExt, path::PathBuf};

use windows_sys::Win32::{
  Foundation::{
    CloseHandle, ERROR_INVALID_PARAMETER, ERROR_NO_MORE_FILES, FILETIME, HANDLE, HWND, INVALID_HANDLE_VALUE, LPARAM,
    SetLastError, WAIT_OBJECT_0, WAIT_TIMEOUT,
  },
  System::{
    Diagnostics::ToolHelp::{
      CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
    },
    Threading::{
      GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE, QueryFullProcessImageNameW,
      TerminateProcess, WaitForSingleObject,
    },
  },
  UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId, PostMessageW, WM_CLOSE},
};

use super::{Entry, Info};

struct Handle(HANDLE);

impl Drop for Handle {
  fn drop(&mut self) {
    // SAFETY: this owns one valid handle, never copied or closed elsewhere.
    if unsafe { CloseHandle(self.0) } == 0 {
      tracing::warn!(error = %io::Error::last_os_error(), "cannot close process inspection handle");
    }
  }
}

pub(super) fn snapshot() -> io::Result<Vec<Entry>> {
  // SAFETY: flags and PID are the documented process snapshot arguments.
  let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
  if raw == INVALID_HANDLE_VALUE {
    return Err(io::Error::last_os_error());
  }
  let handle = Handle(raw);
  // SAFETY: PROCESSENTRY32W is a C record whose fields accept zero.
  let mut entry: PROCESSENTRY32W = unsafe { mem::zeroed() };
  entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as u32;
  let mut result = Vec::new();
  // SAFETY: the snapshot is live and entry has the required size and lifetime.
  let mut found = unsafe { Process32FirstW(handle.0, &mut entry) };
  while found != 0 {
    if entry.th32ProcessID != 0 {
      result.push(Entry {
        pid: entry.th32ProcessID,
        parent: entry.th32ParentProcessID,
      });
    }
    // SAFETY: same live snapshot and correctly sized writable entry.
    found = unsafe { Process32NextW(handle.0, &mut entry) };
  }
  let error = io::Error::last_os_error();
  if error.raw_os_error() == Some(ERROR_NO_MORE_FILES as i32) {
    Ok(result)
  } else {
    Err(error)
  }
}

pub(super) struct Process {
  handle: Handle,
  pub(super) info: Info,
}

impl Process {
  pub(super) fn open(entry: Entry, stopping: bool) -> io::Result<Option<Self>> {
    let rights = PROCESS_QUERY_LIMITED_INFORMATION | 0x0010_0000 | if stopping { PROCESS_TERMINATE } else { 0 };
    // SAFETY: PID and access flags are scalar values; no inherited handle is requested.
    let raw = unsafe { OpenProcess(rights, 0, entry.pid) };
    if raw.is_null() {
      let error = io::Error::last_os_error();
      return if error.raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32) {
        Ok(None)
      } else {
        Err(error)
      };
    }
    let handle = Handle(raw);
    if !running(&handle)? {
      return Ok(None);
    }
    let read = || -> io::Result<(PathBuf, u64)> { Ok((image(&handle)?, start_time(&handle)?)) };
    let (path, started) = match read() {
      Ok(info) => info,
      Err(_) if !running(&handle)? => return Ok(None),
      Err(error) => return Err(error),
    };
    let process = Self {
      handle,
      info: Info {
        pid: entry.pid,
        path,
        started,
      },
    };
    if process.alive()? { Ok(Some(process)) } else { Ok(None) }
  }

  pub(super) fn is_parent_of(&self, entry: Entry) -> bool {
    // The retained handle prevents reuse of the parent's PID even after it exits.
    entry.parent == self.info.pid
  }

  pub(super) fn is_process(&self, entry: Entry) -> bool {
    entry.pid == self.info.pid
  }

  pub(super) fn alive(&self) -> io::Result<bool> {
    running(&self.handle)
  }

  pub(super) fn close(&self) -> io::Result<()> {
    let mut request = CloseRequest {
      process: self,
      error: None,
    };
    // SAFETY: resetting thread-local Win32 last-error distinguishes an empty enumeration from failure.
    unsafe { SetLastError(0) };
    // SAFETY: callback and its stack-owned context live for the synchronous enumeration.
    if unsafe { EnumWindows(Some(close_window), (&mut request as *mut CloseRequest<'_>) as LPARAM) } == 0 {
      let error = request.error.unwrap_or_else(io::Error::last_os_error);
      if error.raw_os_error() != Some(0) {
        return Err(error);
      }
    }
    Ok(())
  }

  pub(super) fn end(&self) -> io::Result<()> {
    if !self.alive()? {
      return Ok(());
    }
    // SAFETY: termination targets the retained, validated process handle, never a freshly looked-up PID.
    if unsafe { TerminateProcess(self.handle.0, 1) } == 0 && self.alive()? {
      return Err(io::Error::last_os_error());
    }
    Ok(())
  }
}

fn running(handle: &Handle) -> io::Result<bool> {
  // SAFETY: the owned process handle remains valid and the zero timeout never blocks.
  match unsafe { WaitForSingleObject(handle.0, 0) } {
    WAIT_OBJECT_0 => Ok(false),
    WAIT_TIMEOUT => Ok(true),
    _ => Err(io::Error::last_os_error()),
  }
}

fn image(handle: &Handle) -> io::Result<PathBuf> {
  let mut buffer = vec![0u16; 32768];
  let mut length = buffer.len() as u32;
  // SAFETY: buffer holds length UTF-16 units, and handle remains live.
  if unsafe { QueryFullProcessImageNameW(handle.0, 0, buffer.as_mut_ptr(), &mut length) } == 0 {
    return Err(io::Error::last_os_error());
  }
  Ok(PathBuf::from(OsString::from_wide(&buffer[..length as usize])))
}

fn start_time(handle: &Handle) -> io::Result<u64> {
  let empty = FILETIME {
    dwLowDateTime: 0,
    dwHighDateTime: 0,
  };
  let (mut start, mut exit, mut kernel, mut user) = (empty, empty, empty, empty);
  // SAFETY: four distinct writable records and a live process handle.
  if unsafe { GetProcessTimes(handle.0, &mut start, &mut exit, &mut kernel, &mut user) } == 0 {
    return Err(io::Error::last_os_error());
  }
  let ticks = (u64::from(start.dwHighDateTime) << 32) | u64::from(start.dwLowDateTime);
  Ok((ticks - 116_444_736_000_000_000) / 10)
}

struct CloseRequest<'a> {
  process: &'a Process,
  error: Option<io::Error>,
}

unsafe extern "system" fn close_window(window: HWND, data: LPARAM) -> i32 {
  // SAFETY: EnumWindows receives this exact stack context, exclusively, for the callback's lifetime.
  let request = unsafe { &mut *(data as *mut CloseRequest<'_>) };
  let mut pid = 0;
  // SAFETY: pid is writable; EnumWindows supplies a top-level HWND.
  unsafe { GetWindowThreadProcessId(window, &mut pid) };
  if pid != request.process.info.pid {
    return 1;
  }
  match request.process.alive() {
    Ok(false) => return 1,
    Ok(true) => {}
    Err(error) => {
      request.error = Some(error);
      return 0;
    }
  }
  // SAFETY: only windows owned by the validated process are targeted; WM_CLOSE carries no pointers.
  if unsafe { PostMessageW(window, WM_CLOSE, 0, 0) } == 0 {
    request.error = Some(io::Error::last_os_error());
    return 0;
  }
  1
}
