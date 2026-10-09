//! Checks that a process is the Orchester app that holds the data folder, finds its main window and
//! brings it to the front: Win32 process and window functions, which have no safe wrapper, so `unsafe` is
//! allowed here and only here.
#![allow(unsafe_code)]

use orchester_core::time::Timestamp;
use windows_sys::{
  Win32::{
    Foundation::{CloseHandle, FILETIME, HANDLE, HWND, LPARAM},
    System::Threading::{
      GetProcessTimes, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    },
    UI::WindowsAndMessaging::{
      EnumWindows, GW_OWNER, GetWindow, GetWindowTextW, GetWindowThreadProcessId, IsHungAppWindow, IsIconic,
      IsWindowVisible, SW_RESTORE, SetForegroundWindow, ShowWindowAsync,
    },
  },
  core::BOOL,
};

use super::Front;

/// What every Orchester program's file name starts with (`orchester.exe`, `orchester-app.exe`).
const PROGRAM_PREFIX: &str = "orchester";
/// The title of lurq's DevTools window, a second top-level window of a development build.
const DEVTOOLS_TITLE: &str = "DevTools";
/// 100-nanosecond intervals from 1601-01-01, where Windows counts file times, to the Unix epoch.
const UNIX_EPOCH_IN_FILE_TIME: u64 = 116_444_736_000_000_000;

/// The process whose window is looked for, and its first visible window without an owner.
struct Search {
  pid: u32,
  found: Option<HWND>,
}

/// Brings the main window of `pid` to the front, if `pid` is still the Orchester program that started
/// before `started_at`: a process id the system gave another program since gets no answer.
pub fn bring_to_front(pid: u32, started_at: Timestamp) -> Front {
  if !is_holder(pid, started_at) {
    return Front::NoAnswer;
  }
  let Some(window) = main_window(pid) else {
    return Front::NoAnswer;
  };
  // SAFETY: `window` is a window handle Windows handed over just now; a window that closed meanwhile is
  // an invalid handle, for which each call fails and returns 0.
  if unsafe { IsHungAppWindow(window) } != 0 {
    return Front::NoAnswer;
  }
  // SAFETY: as above. The asynchronous call never waits for the other process to answer.
  if unsafe { IsIconic(window) } != 0 {
    // SAFETY: as above. Whether the request was posted does not matter: bringing the window to the
    // front below says whether it came.
    unsafe { ShowWindowAsync(window, SW_RESTORE) };
  }
  // SAFETY: as above.
  if unsafe { SetForegroundWindow(window) } == 0 {
    return Front::NoAnswer;
  }
  Front::Brought
}

/// Whether `pid` is an Orchester program created before `started_at`, when its session started.
fn is_holder(pid: u32, started_at: Timestamp) -> bool {
  // SAFETY: opening a process by id only reads; a null handle is the failure, checked below.
  let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
  if process.is_null() {
    return false;
  }
  let holder = created_before(process, started_at) && is_orchester(process);
  // SAFETY: `process` is the handle opened above, closed once. Closing a handle this process owns does
  // not fail in a way that matters here.
  unsafe { CloseHandle(process) };
  holder
}

fn created_before(process: HANDLE, started_at: Timestamp) -> bool {
  let empty = FILETIME {
    dwLowDateTime: 0,
    dwHighDateTime: 0,
  };
  let (mut created, mut exited, mut kernel, mut user) = (empty, empty, empty, empty);
  // SAFETY: `process` is an open handle with query rights; the four times are live `FILETIME`s the call
  // only writes.
  let read = unsafe {
    GetProcessTimes(
      process,
      &raw mut created,
      &raw mut exited,
      &raw mut kernel,
      &raw mut user,
    )
  };
  if read == 0 {
    return false;
  }
  let ticks = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
  let millis = ticks.saturating_sub(UNIX_EPOCH_IN_FILE_TIME) / 10_000;
  i64::try_from(millis).is_ok_and(|millis| millis <= started_at.unix_millis())
}

fn is_orchester(process: HANDLE) -> bool {
  let mut path = [0_u16; 1024];
  let mut length = u32::try_from(path.len()).expect("the buffer's length fits in u32");
  // SAFETY: `path` is a live buffer of `length` UTF-16 units, which the call writes and then sets
  // `length` to the units it wrote.
  let read = unsafe { QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, path.as_mut_ptr(), &raw mut length) };
  if read == 0 {
    return false;
  }
  let path = String::from_utf16_lossy(&path[..length as usize]);
  let name = path
    .rsplit(['\\', '/'])
    .next()
    .expect("splitting yields at least one piece");
  name.to_lowercase().starts_with(PROGRAM_PREFIX)
}

fn main_window(pid: u32) -> Option<HWND> {
  let mut search = Search { pid, found: None };
  // SAFETY: `visit` reads only the windows handed to it and writes `search`, which outlives the call
  // and is passed by its address. EnumWindows returns 0 also when `visit` stops it early, so its result
  // does not say whether a window was found; `search.found` does.
  unsafe { EnumWindows(Some(visit), (&raw mut search) as LPARAM) };
  search.found
}

/// Keeps enumerating until a visible window without an owner of the searched process turns up that is
/// not lurq's DevTools.
unsafe extern "system" fn visit(window: HWND, search: LPARAM) -> BOOL {
  // SAFETY: `search` is the address of the `Search` that `main_window` lends EnumWindows for the whole
  // enumeration, and nothing else touches it meanwhile.
  let search = unsafe { &mut *(search as *mut Search) };
  let mut pid = 0;
  // SAFETY: `window` is a top-level window EnumWindows hands over; `pid` is a live `u32` the call writes.
  unsafe { GetWindowThreadProcessId(window, &raw mut pid) };
  // SAFETY: as above.
  let main = pid == search.pid
    && unsafe { IsWindowVisible(window) } != 0
    && unsafe { GetWindow(window, GW_OWNER) }.is_null()
    && !title(window).contains(DEVTOOLS_TITLE);
  if main {
    search.found = Some(window);
    return 0;
  }
  1
}

fn title(window: HWND) -> String {
  let mut text = [0_u16; 256];
  let capacity = i32::try_from(text.len()).expect("the buffer's length fits in i32");
  // SAFETY: `text` is a live buffer of `capacity` UTF-16 units; the call writes at most that many and
  // returns how many it wrote, 0 for none or a failure.
  let length = unsafe { GetWindowTextW(window, text.as_mut_ptr(), capacity) };
  // A failure returns 0, never a negative count.
  // A failure returns 0, never a negative count.
  String::from_utf16_lossy(&text[..usize::try_from(length).unwrap_or(0)])
}
