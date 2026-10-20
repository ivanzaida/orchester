//! The *Needs you* count over the app's taskbar button: an overlay icon (`ITaskbarList3::SetOverlayIcon`) drawn here
//! as a disc with the count, `9+` past nine, at the system's small icon size, and none at 0. Its description is what
//! a screen reader says. GDI drawing and the taskbar's COM interface have no safe wrapper, so `unsafe` is allowed
//! here and only here.
#![allow(unsafe_code)]

use std::{ffi::c_void, ptr, slice};

use windows::{
  Win32::{
    Foundation::{COLORREF, HWND, RECT},
    Graphics::Gdi::{
      ANTIALIASED_QUALITY, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CLIP_DEFAULT_PRECIS, CreateBitmap, CreateCompatibleDC,
      CreateDIBSection, CreateFontW, DEFAULT_CHARSET, DIB_RGB_COLORS, DT_CENTER, DT_SINGLELINE, DT_VCENTER, DeleteDC,
      DeleteObject, DrawTextW, FF_SWISS, FW_BOLD, GdiFlush, HDC, HGDIOBJ, OUT_DEFAULT_PRECIS, SelectObject, SetBkMode,
      SetTextColor, TRANSPARENT,
    },
    System::Com::{CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize},
    UI::{
      Shell::{ITaskbarList3, TaskbarList},
      WindowsAndMessaging::{CreateIconIndirect, DestroyIcon, GetSystemMetrics, HICON, ICONINFO, SM_CXSMICON},
    },
  },
  core::{HSTRING, w},
};

use super::super::OsError;
use crate::{system_icon, theme::badge};

/// Shows `count` over the taskbar button of each of the app's windows, described as `label`; removes it at 0.
/// `false` while the app has no window with a taskbar button yet.
pub fn set(count: usize, label: &str) -> Result<bool, OsError> {
  let windows = system_icon::windows_with_icon();
  if windows.is_empty() {
    return Ok(false);
  }
  // Declared before the taskbar, so the taskbar is released before the apartment's count is given back.
  let _apartment = Apartment::enter();
  // SAFETY: a class id and an interface the system provides, created in this process.
  let taskbar: ITaskbarList3 = unsafe { CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER) }?;
  // SAFETY: the interface was just created.
  unsafe { taskbar.HrInit() }?;
  let icon = if count == 0 { None } else { Some(Icon::of(count)?) };
  let description = HSTRING::from(label);
  for window in windows {
    let handle = icon.as_ref().map_or_else(HICON::default, |icon| icon.0);
    // SAFETY: `window` is a window of this thread, enumerated just now; the taskbar copies the icon.
    unsafe { taskbar.SetOverlayIcon(HWND(window), handle, &description) }?;
  }
  Ok(true)
}

/// The UI thread's COM apartment while the badge is set: entered once more, and left again when dropped. COM is set
/// up on the UI thread already (winit's drag and drop), so entering it in the same mode only counts again and is
/// given back on drop; one in another mode enters nothing and gives nothing back. Either way the taskbar says
/// whether it answers.
struct Apartment {
  entered: bool,
}

impl Apartment {
  fn enter() -> Self {
    // SAFETY: no pointers; called on the UI thread, which owns its apartment.
    let entered = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
    Self { entered }
  }
}

impl Drop for Apartment {
  fn drop(&mut self) {
    if self.entered {
      // SAFETY: pairs the successful `CoInitializeEx` of `enter`, on the same thread.
      unsafe { CoUninitialize() };
    }
  }
}

/// An icon this module drew, destroyed when it is dropped.
struct Icon(HICON);

impl Drop for Icon {
  fn drop(&mut self) {
    // SAFETY: the icon was created by `Icon::of` and is destroyed once; the taskbar keeps its own copy.
    if let Err(error) = unsafe { DestroyIcon(self.0) } {
      tracing::debug!(%error, "the badge's icon could not be destroyed");
    }
  }
}

impl Icon {
  /// A disc with `count`, `9+` past nine, at the small icon size.
  fn of(count: usize) -> Result<Self, OsError> {
    let figures = if count > 9 { "9+".to_owned() } else { count.to_string() };
    // SAFETY: a value query without pointers.
    let size = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16);
    // SAFETY: a memory DC compatible with the screen, deleted below.
    let dc = unsafe { CreateCompatibleDC(None) };
    let drawn = draw(dc, size, &figures);
    // SAFETY: `dc` was created above, and every object selected into it was selected out by `draw`.
    unsafe { DeleteDC(dc) }.ok()?;
    drawn
  }
}

/// Draws the badge into a new 32-bit bitmap selected into `dc`, and makes it an icon.
fn draw(dc: HDC, size: i32, figures: &str) -> Result<Icon, OsError> {
  let info = BITMAPINFO {
    bmiHeader: BITMAPINFOHEADER {
      biSize: u32::try_from(size_of::<BITMAPINFOHEADER>()).expect("a header's size fits in u32"),
      biWidth: size,
      // Negative: rows from the top.
      biHeight: -size,
      biPlanes: 1,
      biBitCount: 32,
      biCompression: BI_RGB.0,
      ..Default::default()
    },
    ..Default::default()
  };
  let mut bits: *mut c_void = ptr::null_mut();
  // SAFETY: `info` describes a top-down 32-bit bitmap; the call points `bits` at its pixels, which live as long as
  // the bitmap.
  let color = unsafe { CreateDIBSection(Some(dc), &raw const info, DIB_RGB_COLORS, &raw mut bits, None, 0) }?;
  let side = usize::try_from(size).expect("the small icon size is positive");
  // SAFETY: the bitmap holds `side * side` 32-bit pixels at `bits`, and nothing else writes them while this does.
  let pixels = unsafe { slice::from_raw_parts_mut(bits.cast::<u32>(), side * side) };
  disc(pixels, side, false);
  write_figures(dc, (color.into(), size), figures);
  disc(pixels, side, true);
  // SAFETY: a monochrome mask of the icon's size; the color bitmap's alpha decides what shows.
  let mask = unsafe { CreateBitmap(size, size, 1, 1, None) };
  let parts = ICONINFO {
    fIcon: true.into(),
    xHotspot: 0,
    yHotspot: 0,
    hbmMask: mask,
    hbmColor: color,
  };
  // SAFETY: both bitmaps are live; the icon copies them, so they are deleted after.
  let icon = unsafe { CreateIconIndirect(&raw const parts) };
  // SAFETY: the bitmaps were created above, are selected into no DC now, and are deleted once.
  unsafe {
    let _mask_deleted = DeleteObject(mask.into());
    let _color_deleted = DeleteObject(color.into());
  }
  Ok(Icon(icon?))
}

/// Fills the disc: its pixels in the disc's color, opaque, the rest transparent; with `keep` only makes the disc
/// opaque again over the figures GDI wrote into it, which it leaves without alpha.
fn disc(pixels: &mut [u32], side: usize, keep: bool) {
  let radius = side as f32 / 2.0;
  for (index, pixel) in pixels.iter_mut().enumerate() {
    let (x, y) = (
      (index % side) as f32 + 0.5 - radius,
      (index / side) as f32 + 0.5 - radius,
    );
    let inside = x * x + y * y <= radius * radius;
    *pixel = match (inside, keep) {
      (false, _) => 0,
      (true, false) => 0xFF00_0000 | badge::DISC,
      (true, true) => 0xFF00_0000 | (*pixel & 0x00FF_FFFF),
    };
  }
}

/// Writes `figures` centered on the bitmap `color` of `size`, selected into `dc` while it writes.
fn write_figures(dc: HDC, (color, size): (HGDIOBJ, i32), figures: &str) {
  // GDI takes 0x00BBGGRR; the theme gives 0xRRGGBB.
  let rgb = badge::FIGURES;
  let text = COLORREF(((rgb & 0xFF) << 16) | (rgb & 0xFF00) | ((rgb >> 16) & 0xFF));
  // SAFETY: `dc` is a live memory DC; every object selected into it is selected out again before this returns, and
  // the font is deleted once. `wide` and `rect` live through the call that reads and writes them.
  unsafe {
    let before = SelectObject(dc, color);
    let font = CreateFontW(
      -(size * 3 / 4),
      0,
      0,
      0,
      FW_BOLD.0 as i32,
      0,
      0,
      0,
      DEFAULT_CHARSET,
      OUT_DEFAULT_PRECIS,
      CLIP_DEFAULT_PRECIS,
      ANTIALIASED_QUALITY,
      FF_SWISS.0 as u32,
      w!("Segoe UI"),
    );
    let before_font = SelectObject(dc, font.into());
    SetBkMode(dc, TRANSPARENT);
    SetTextColor(dc, text);
    let mut wide: Vec<u16> = figures.encode_utf16().collect();
    let mut rect = RECT {
      left: 0,
      top: 0,
      right: size,
      bottom: size,
    };
    DrawTextW(dc, &mut wide, &raw mut rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    let _flushed = GdiFlush();
    SelectObject(dc, before_font);
    SelectObject(dc, before);
    let _font_deleted = DeleteObject(font.into());
  }
}
