//! Finds a process's main window and brings it to the front: Win32 window functions, which have no safe
//! wrapper, so `unsafe` is allowed here and only here.
#![allow(unsafe_code)]

use windows_sys::{
  Win32::{
    Foundation::{HWND, LPARAM},
    UI::WindowsAndMessaging::{
      EnumWindows, GW_OWNER, GetWindow, GetWindowThreadProcessId, IsHungAppWindow, IsIconic, IsWindowVisible,
      SW_RESTORE, SetForegroundWindow, ShowWindowAsync,
    },
  },
  core::BOOL,
};

use super::Front;

/// The process whose window is looked for, and its first visible window without an owner.
struct Search {
  pid: u32,
  found: Option<HWND>,
}

pub fn bring_to_front(pid: u32) -> Front {
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

fn main_window(pid: u32) -> Option<HWND> {
  let mut search = Search { pid, found: None };
  // SAFETY: `visit` reads only the windows handed to it and writes `search`, which outlives the call
  // and is passed by its address. EnumWindows returns 0 also when `visit` stops it early, so its result
  // does not say whether a window was found; `search.found` does.
  unsafe { EnumWindows(Some(visit), (&raw mut search) as LPARAM) };
  search.found
}

/// Keeps enumerating until a visible window without an owner of the searched process turns up.
unsafe extern "system" fn visit(window: HWND, search: LPARAM) -> BOOL {
  // SAFETY: `search` is the address of the `Search` that `main_window` lends EnumWindows for the whole
  // enumeration, and nothing else touches it meanwhile.
  let search = unsafe { &mut *(search as *mut Search) };
  let mut pid = 0;
  // SAFETY: `window` is a top-level window EnumWindows hands over; `pid` is a live `u32` the call writes.
  unsafe { GetWindowThreadProcessId(window, &raw mut pid) };
  // SAFETY: as above.
  let main =
    pid == search.pid && unsafe { IsWindowVisible(window) } != 0 && unsafe { GetWindow(window, GW_OWNER) }.is_null();
  if main {
    search.found = Some(window);
    return 0;
  }
  1
}
