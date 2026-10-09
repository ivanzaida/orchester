//! Gives the window class and the windows of the UI thread the executable's embedded icon. This is FFI
//! with no safe wrapper available (neither lurq nor winit loads icon resources or sets class icons), so
//! `unsafe` is allowed here and only here.
#![allow(unsafe_code)]

use std::{io, ptr};

use windows_sys::{
  Win32::{
    Foundation::{HWND, LPARAM},
    System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
    UI::WindowsAndMessaging::{
      EnumThreadWindows, GCLP_HICON, GCLP_HICONSM, GET_CLASS_LONG_INDEX, GetClassLongPtrW, GetSystemMetrics, HICON,
      ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_DEFAULTCOLOR, LoadImageW, SM_CXICON, SM_CXSMICON, SM_CYICON, SM_CYSMICON,
      SYSTEM_METRICS_INDEX, SendMessageW, SetClassLongPtrW, WM_GETICON, WM_SETICON,
    },
  },
  core::BOOL,
};

use super::SystemIconError;

/// The icon group of `assets/icon/orchester.rc`: `1 ICON "assets/icon/orchester.ico"`.
const ICON_RESOURCE: usize = 1;

/// Gives each of this thread's windows that carries a window icon (lurq's), and its class, the
/// executable's icon: the big one (taskbar button, Alt+Tab) and the small one (thumbnail) at the
/// system's sizes. Runs on the UI thread, once the first frame is presented: setting the window's icon
/// again then also has the taskbar read it anew from a window that answers.
pub fn install() -> Result<(), SystemIconError> {
  let big = embedded_icon(SM_CXICON, SM_CYICON)?;
  let small = embedded_icon(SM_CXSMICON, SM_CYSMICON)?;
  let windows = windows_with_icon();
  if windows.is_empty() {
    return Err(SystemIconError::NoWindow);
  }
  for window in windows {
    set_class_icon(window, GCLP_HICON, big)?;
    set_class_icon(window, GCLP_HICONSM, small)?;
    set_window_icon(window, ICON_BIG, big);
    set_window_icon(window, ICON_SMALL, small);
  }
  Ok(())
}

/// The executable's icon at the size of two system metrics, the frame Windows picks from
/// `orchester.ico`. It is never destroyed: the class and the window use it while the process runs.
fn embedded_icon(width: SYSTEM_METRICS_INDEX, height: SYSTEM_METRICS_INDEX) -> Result<HICON, SystemIconError> {
  // SAFETY: value queries without pointers.
  let (width, height) = unsafe { (GetSystemMetrics(width), GetSystemMetrics(height)) };
  // SAFETY: a null name asks for the module of the process's executable, which is never unloaded.
  let module = unsafe { GetModuleHandleW(ptr::null()) };
  // SAFETY: `module` is the executable, and a resource ID in place of a name is how resource functions
  // take numbered resources (`MAKEINTRESOURCEW`); no other pointer is passed.
  let icon = unsafe {
    LoadImageW(
      module,
      ptr::without_provenance(ICON_RESOURCE),
      IMAGE_ICON,
      width,
      height,
      LR_DEFAULTCOLOR,
    )
  };
  if icon.is_null() {
    return Err(SystemIconError::Load {
      size: width,
      source: io::Error::last_os_error(),
    });
  }
  Ok(icon)
}

/// This thread's top-level windows that carry a big window icon: the ones lurq gave theirs, not winit's
/// hidden event window or the input method's.
fn windows_with_icon() -> Vec<HWND> {
  let mut windows: Vec<HWND> = Vec::new();
  let list = (&raw mut windows).expose_provenance() as LPARAM;
  // SAFETY: `collect` runs only during this call, and only pushes onto `windows`, whose address `list`
  // holds and which outlives the call. The result reports no error: it is false when the thread has no
  // windows, which the empty list shows.
  unsafe { EnumThreadWindows(GetCurrentThreadId(), Some(collect), list) };
  windows.retain(|&window| {
    // SAFETY: `window` is a window of this thread, enumerated just now; its window procedure answers.
    unsafe { SendMessageW(window, WM_GETICON, ICON_BIG as usize, 0) != 0 }
  });
  windows
}

/// Adds each enumerated window to the `Vec<HWND>` that `list` points to, and goes on (`TRUE`).
unsafe extern "system" fn collect(window: HWND, list: LPARAM) -> BOOL {
  // SAFETY: `windows_with_icon` passes the address of its list, alive and borrowed by nothing else while
  // the enumeration runs.
  let windows = unsafe { &mut *ptr::with_exposed_provenance_mut::<Vec<HWND>>(list as usize) };
  windows.push(window);
  1
}

fn set_class_icon(window: HWND, which: GET_CLASS_LONG_INDEX, icon: HICON) -> Result<(), SystemIconError> {
  // SAFETY: `window` is a window of this thread; its class keeps `icon`, which is never destroyed. The
  // result, the icon it replaces, tells nothing: it is 0 before (winit registers its class without
  // icons), and the call leaves a stale last error even when it succeeds, so success is read back.
  unsafe { SetClassLongPtrW(window, which, icon as isize) };
  // SAFETY: as above; a value query.
  if unsafe { GetClassLongPtrW(window, which) } != icon as usize {
    return Err(SystemIconError::SetClassIcon(io::Error::last_os_error()));
  }
  Ok(())
}

fn set_window_icon(window: HWND, which: u32, icon: HICON) {
  // SAFETY: `window` is a window of this thread; it keeps `icon`, which is never destroyed. The message
  // returns the icon it replaces, lurq's, which winit still owns and frees.
  unsafe { SendMessageW(window, WM_SETICON, which as usize, icon as LPARAM) };
}
