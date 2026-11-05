#![allow(unsafe_code)]
//! Win32 windows of the run's processes have no std or safe wrapper: listing the top-level windows of given
//! processes, moving one off-screen without activating it, waiting until its thread handled what it was sent, and
//! its own rendering into memory (`PrintWindow`). Nothing here reads the screen or sends input.

use std::{ffi::c_void, mem, ptr};

use windows::{
  Win32::{
    Foundation::{HWND, LPARAM, RECT, WPARAM},
    Graphics::Gdi::{
      BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC,
      DeleteObject, HBITMAP, HDC, HGDIOBJ, MONITOR_DEFAULTTONULL, MonitorFromWindow, SelectObject,
    },
    Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow},
    UI::WindowsAndMessaging::{
      EnumWindows, GetSystemMetrics, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
      PW_RENDERFULLCONTENT, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SMTO_ABORTIFHUNG, SWP_NOACTIVATE, SWP_NOOWNERZORDER,
      SWP_NOSIZE, SWP_NOZORDER, SendMessageTimeoutW, SetWindowPos, WM_NULL,
    },
  },
  core::BOOL,
};

use super::super::{Bounds, NativeWindow};

/// How far beyond the left and top of the desktop a window is placed.
const OFF_SCREEN_MARGIN: i32 = 64;
/// The longest title read.
const TITLE_UNITS: usize = 512;

/// The visible top-level windows whose process is one of `pids`, in the order the desktop lists them (front first).
pub(super) fn of_processes(pids: &[u32]) -> Result<Vec<NativeWindow>, String> {
  let mut found = Found {
    pids,
    windows: Vec::new(),
  };
  // SAFETY: the callback receives this stack record, exclusively, only during this synchronous enumeration.
  unsafe { EnumWindows(Some(listed), LPARAM(ptr::from_mut(&mut found) as isize)) }.map_err(|error| error.message())?;
  Ok(found.windows)
}

struct Found<'a> {
  pids: &'a [u32],
  windows: Vec<NativeWindow>,
}

unsafe extern "system" fn listed(window: HWND, data: LPARAM) -> BOOL {
  // SAFETY: `of_processes` passes its own `Found`, alive and not otherwise borrowed during the enumeration.
  let found = unsafe { &mut *(data.0 as *mut Found<'_>) };
  let mut pid = 0;
  // SAFETY: EnumWindows hands a top-level window; pid is a writable u32.
  unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) };
  // SAFETY: a plain query of the handed window.
  if !found.pids.contains(&pid) || !unsafe { IsWindowVisible(window) }.as_bool() {
    return BOOL(1);
  }
  if let Ok(bounds) = bounds(window) {
    found.windows.push(NativeWindow {
      handle: window.0 as isize,
      pid,
      title: title(window),
      bounds,
      off_screen: off_screen(window),
    });
  }
  BOOL(1)
}

fn hwnd(window: &NativeWindow) -> HWND {
  HWND(window.handle as *mut c_void)
}

fn title(window: HWND) -> String {
  let mut buffer = [0u16; TITLE_UNITS];
  // SAFETY: the buffer is writable for its whole length, which the call is given.
  let length = unsafe { GetWindowTextW(window, &mut buffer) };
  String::from_utf16_lossy(&buffer[..usize::try_from(length).unwrap_or(0)])
}

fn bounds(window: HWND) -> Result<Bounds, String> {
  let mut rect = RECT::default();
  // SAFETY: rect is a writable RECT.
  unsafe { GetWindowRect(window, &mut rect) }.map_err(|error| error.message())?;
  Ok(Bounds {
    x: rect.left,
    y: rect.top,
    width: rect.right - rect.left,
    height: rect.bottom - rect.top,
  })
}

/// Whether no monitor shows any part of the window.
fn off_screen(window: HWND) -> bool {
  // SAFETY: a plain query of a window handle; no monitor is created.
  unsafe { MonitorFromWindow(window, MONITOR_DEFAULTTONULL) }.is_invalid()
}

/// Moves `window` beyond the top left of the desktop, its size and its place in the z-order kept, without
/// activating it; the window as it is then.
pub(super) fn place_off_screen(window: &NativeWindow) -> Result<NativeWindow, String> {
  if window.off_screen {
    return Ok(window.clone());
  }
  // SAFETY: plain metric queries.
  let (left, top) = unsafe { (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_YVIRTUALSCREEN)) };
  let (x, y) = (
    left - window.bounds.width - OFF_SCREEN_MARGIN,
    top - window.bounds.height - OFF_SCREEN_MARGIN,
  );
  let flags = SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOOWNERZORDER;
  // SAFETY: the handle is of a window of a checked process of the run; the flags keep its size, order and focus.
  unsafe { SetWindowPos(hwnd(window), None, x, y, 0, 0, flags) }.map_err(|error| error.message())?;
  let handle = hwnd(window);
  Ok(NativeWindow {
    bounds: bounds(handle)?,
    off_screen: off_screen(handle),
    ..window.clone()
  })
}

/// Waits up to `timeout_ms` until the window's thread handled a message that does nothing (`WM_NULL`): what it was
/// busy with before is done. `false` when it did not in time.
pub(super) fn settled(window: &NativeWindow, timeout_ms: u32) -> bool {
  // SAFETY: WM_NULL carries no pointers and changes nothing; the call gives up after the timeout or on a hung window.
  let answered = unsafe {
    SendMessageTimeoutW(
      hwnd(window),
      WM_NULL,
      WPARAM(0),
      LPARAM(0),
      SMTO_ABORTIFHUNG,
      timeout_ms,
      None,
    )
  };
  answered.0 != 0
}

/// The window's own rendering of itself: `(width, height, BGRA rows from the top)`.
pub(super) fn render(window: &NativeWindow) -> Result<(u32, u32, Vec<u8>), String> {
  let handle = hwnd(window);
  let size = bounds(handle)?;
  let (width, height) = (
    u32::try_from(size.width).map_err(|_| "the window has no width".to_owned())?,
    u32::try_from(size.height).map_err(|_| "the window has no height".to_owned())?,
  );
  if width == 0 || height == 0 {
    return Err("the window has no area".to_owned());
  }
  let memory = MemoryDc::new()?;
  let mut bits: *mut c_void = ptr::null_mut();
  let info = BITMAPINFO {
    bmiHeader: BITMAPINFOHEADER {
      biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
      biWidth: size.width,
      // Negative: rows from the top.
      biHeight: -size.height,
      biPlanes: 1,
      biBitCount: 32,
      biCompression: BI_RGB.0,
      ..Default::default()
    },
    ..Default::default()
  };
  // SAFETY: info describes a 32-bit top-down bitmap; bits receives the section's memory, owned by the bitmap.
  let bitmap = unsafe { CreateDIBSection(Some(memory.0), &info, DIB_RGB_COLORS, &mut bits, None, 0) }
    .map_err(|error| error.message())?;
  let bitmap = Bitmap(bitmap);
  // SAFETY: both handles are live and owned here; the previous object is put back before the DC goes.
  let previous = unsafe { SelectObject(memory.0, HGDIOBJ(bitmap.0.0)) };
  // SAFETY: the window draws itself into the memory DC; the screen is never read.
  let drawn = unsafe { PrintWindow(handle, memory.0, PRINT_WINDOW_FLAGS(PW_RENDERFULLCONTENT)) }.as_bool();
  let length = width as usize * height as usize * 4;
  // SAFETY: the section holds width × height 32-bit pixels and lives until `bitmap` drops, after this copy.
  let pixels =
    (drawn && !bits.is_null()).then(|| unsafe { std::slice::from_raw_parts(bits.cast::<u8>(), length) }.to_vec());
  // SAFETY: restores the DC's own bitmap before both are deleted.
  unsafe { SelectObject(memory.0, previous) };
  pixels
    .map(|pixels| (width, height, pixels))
    .ok_or_else(|| "the window did not draw itself".to_owned())
}

/// A memory device context, compatible with the display but drawing only into its own bitmap.
struct MemoryDc(HDC);

impl MemoryDc {
  fn new() -> Result<Self, String> {
    // SAFETY: a memory DC with no window; deleted on drop.
    let dc = unsafe { CreateCompatibleDC(None) };
    if dc.is_invalid() {
      return Err("no memory device context".to_owned());
    }
    Ok(Self(dc))
  }
}

impl Drop for MemoryDc {
  fn drop(&mut self) {
    // SAFETY: this owns the DC, created by CreateCompatibleDC and deleted once.
    if !unsafe { DeleteDC(self.0) }.as_bool() {
      tracing::warn!("cannot delete a memory device context");
    }
  }
}

struct Bitmap(HBITMAP);

impl Drop for Bitmap {
  fn drop(&mut self) {
    // SAFETY: this owns the bitmap, no longer selected into any DC, deleted once.
    if !unsafe { DeleteObject(HGDIOBJ(self.0.0)) }.as_bool() {
      tracing::warn!("cannot delete a capture bitmap");
    }
  }
}
