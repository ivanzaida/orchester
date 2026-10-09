//! The running app gives Windows its icon: `orchester-app`, started without taking the focus (so the
//! test leaves the person's keyboard alone), soon has the icon embedded in its executable at the
//! system's sizes as its window icons (taskbar button, Alt+Tab, thumbnail) and as its window class's
//! icons, which the taskbar takes when the window does not answer. macOS takes an app's icon from its
//! bundle instead (`system_icon` reports that it has none); there is nothing to read there.
//!
//! Reading another process's window is FFI with no safe wrapper available, so `unsafe` is allowed in
//! this test, each block with its `// SAFETY:` comment.
#![cfg(windows)]
#![allow(unsafe_code)]

use std::{
  process::{Child, Command},
  thread,
  time::{Duration, Instant},
};

use windows_sys::{
  Win32::{
    Foundation::{HWND, LPARAM},
    Graphics::Gdi::{BITMAP, DeleteObject, GetObjectW},
    UI::{
      HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext},
      WindowsAndMessaging::{
        EnumWindows, GCLP_HICON, GCLP_HICONSM, GET_CLASS_LONG_INDEX, GWL_EXSTYLE, GetClassLongPtrW, GetIconInfo,
        GetSystemMetrics, GetWindowLongPtrW, GetWindowThreadProcessId, HICON, ICON_BIG, ICON_SMALL, ICONINFO,
        IsWindowVisible, PostMessageW, SM_CXICON, SM_CXSMICON, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_CLOSE,
        WM_GETICON, WS_EX_TOOLWINDOW,
      },
    },
  },
  core::BOOL,
};

/// Long enough for a debug build to start and draw its first frame on a busy machine.
const PATIENCE: Duration = Duration::from_secs(60);

/// The started app; dropping it ends that process (only that one) if it is still running.
struct Started(Child);

impl Drop for Started {
  fn drop(&mut self) {
    if matches!(self.0.try_wait(), Ok(None)) {
      self.0.kill().expect("the test ends the app it started");
      self.0.wait().expect("the ended app is reaped");
    }
  }
}

#[test]
fn system_icon_reaches_the_window_and_its_class() {
  let mut app = Started(
    Command::new(env!("CARGO_BIN_EXE_orchester-app"))
      .env("ORCHESTER_APP_MCP_START_WITHOUT_FOCUS", "1")
      .spawn()
      .expect("the app starts"),
  );
  let window = eventually(|| taskbar_window(app.0.id())).expect("the app shows a window on the taskbar");
  let big = eventually(|| class_icon(window, GCLP_HICON)).expect("the window class gets a big icon");
  let small = eventually(|| class_icon(window, GCLP_HICONSM)).expect("the window class gets a small icon");

  // SAFETY: no pointers. As DPI aware as the app (winit makes it so), the test thread reads the
  // system's icon sizes at the system's scale, as the app does.
  let (big_side, small_side) = unsafe {
    SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    (GetSystemMetrics(SM_CXICON), GetSystemMetrics(SM_CXSMICON))
  };
  assert_eq!(icon_size(big), Some((big_side, big_side)), "class icon");
  assert_eq!(icon_size(small), Some((small_side, small_side)), "small class icon");
  assert_eq!(window_icon(window, ICON_BIG), Some(big), "the window's own big icon");
  assert_eq!(
    window_icon(window, ICON_SMALL),
    Some(small),
    "the window's own small icon"
  );

  // SAFETY: `window` is the app's window; WM_CLOSE carries no pointers.
  unsafe { PostMessageW(window, WM_CLOSE, 0, 0) };
  let closed = eventually(|| app.0.try_wait().ok().flatten());
  assert!(
    closed.is_some_and(|status| status.success()),
    "the app closes from its window"
  );
}

fn eventually<T>(mut probe: impl FnMut() -> Option<T>) -> Option<T> {
  let deadline = Instant::now() + PATIENCE;
  loop {
    if let Some(found) = probe() {
      return Some(found);
    }
    if Instant::now() > deadline {
      return None;
    }
    thread::sleep(Duration::from_millis(50));
  }
}

/// The visible window of process `pid` that has a taskbar button: not a tool window (winit's event
/// window, a console's).
fn taskbar_window(pid: u32) -> Option<HWND> {
  let mut found: (u32, Option<HWND>) = (pid, None);
  let search = (&raw mut found).expose_provenance() as LPARAM;
  // SAFETY: `match_window` runs only during this call and only touches `found`, whose address `search`
  // holds and which outlives the call. The result only says whether the search stopped early.
  unsafe { EnumWindows(Some(match_window), search) };
  found.1
}

unsafe extern "system" fn match_window(window: HWND, search: LPARAM) -> BOOL {
  // SAFETY: `taskbar_window` passes the address of its `(pid, found)`, alive and borrowed by nothing
  // else during the enumeration.
  let (pid, found) = unsafe { &mut *std::ptr::with_exposed_provenance_mut::<(u32, Option<HWND>)>(search as usize) };
  let mut owner = 0;
  // SAFETY: `window` comes from the enumeration; `owner` is a live, writable u32.
  let visible = unsafe {
    GetWindowThreadProcessId(window, &mut owner);
    IsWindowVisible(window) != 0
  };
  // SAFETY: as above; a value query.
  let tool = (unsafe { GetWindowLongPtrW(window, GWL_EXSTYLE) } as u32 & WS_EX_TOOLWINDOW) != 0;
  if owner == *pid && visible && !tool {
    *found = Some(window);
    return 0;
  }
  1
}

fn class_icon(window: HWND, which: GET_CLASS_LONG_INDEX) -> Option<HICON> {
  // SAFETY: a value query of a window that lives until the test closes it.
  let icon = unsafe { GetClassLongPtrW(window, which) };
  (icon != 0).then_some(icon as HICON)
}

fn window_icon(window: HWND, which: u32) -> Option<HICON> {
  let mut icon = 0;
  // SAFETY: `window` lives until the test closes it; `icon` is a live, writable usize; WM_GETICON
  // carries no pointers.
  let answered =
    unsafe { SendMessageTimeoutW(window, WM_GETICON, which as usize, 0, SMTO_ABORTIFHUNG, 5000, &mut icon) };
  (answered != 0 && icon != 0).then_some(icon as HICON)
}

/// The width and height of an icon's color image.
fn icon_size(icon: HICON) -> Option<(i32, i32)> {
  // SAFETY: ICONINFO is plain data, valid zeroed; GetIconInfo fills it.
  let mut info: ICONINFO = unsafe { std::mem::zeroed() };
  // SAFETY: `icon` is a live icon handle; `info` is live and writable.
  if unsafe { GetIconInfo(icon, &mut info) } == 0 {
    return None;
  }
  // SAFETY: BITMAP is plain data, valid zeroed; GetObjectW fills it.
  let mut bitmap: BITMAP = unsafe { std::mem::zeroed() };
  let length = i32::try_from(size_of::<BITMAP>()).expect("BITMAP is small");
  // SAFETY: `hbmColor` is the bitmap GetIconInfo created for this call; `bitmap` is live, writable and
  // `length` bytes long. Both bitmaps GetIconInfo created are deleted, the result only says whether
  // they were.
  let read = unsafe {
    let read = GetObjectW(info.hbmColor, length, (&raw mut bitmap).cast());
    DeleteObject(info.hbmColor);
    DeleteObject(info.hbmMask);
    read
  };
  (read != 0).then_some((bitmap.bmWidth, bitmap.bmHeight))
}
