#![allow(unsafe_code)]
//! The window server (CoreGraphics) has no safe wrapper in the build: the windows of given processes with their ids
//! and bounds, the displays' bounds, whether macOS lets this process capture windows, one window's image by its id
//! where ScreenCaptureKit cannot take it, and an image's pixels. Nothing here reads the whole screen.

use std::ffi::c_void;

use super::{
  super::{Bounds, NativeCapture},
  cf::{Cf, CfRef},
};

/// `kCGWindowListOptionAll | kCGWindowListExcludeDesktopElements`: on-screen and off-screen windows, without the
/// desktop's own.
const ALL_WINDOWS: u32 = 1 << 4;
/// `kCGWindowListOptionIncludingWindow`: that one window only.
const ONE_WINDOW: u32 = 1 << 3;
/// `kCGWindowImageBoundsIgnoreFraming | kCGWindowImageBestResolution`: without the shadow, in pixels.
const IMAGE_OPTIONS: u32 = 1 | (1 << 3);
/// `kCGImageAlphaPremultipliedLast | kCGBitmapByteOrder32Big`: RGBA bytes in this order.
const RGBA: u32 = 1 | (4 << 12);
/// The most displays read.
const DISPLAYS: usize = 32;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Rect {
  x: f64,
  y: f64,
  width: f64,
  height: f64,
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
  static kCGWindowNumber: CfRef;
  static kCGWindowOwnerPID: CfRef;
  static kCGWindowLayer: CfRef;
  static kCGWindowBounds: CfRef;
  static kCGWindowName: CfRef;
  static CGRectNull: Rect;
  fn CGWindowListCopyWindowInfo(option: u32, relative_to: u32) -> CfRef;
  fn CGRectMakeWithDictionaryRepresentation(dictionary: CfRef, rect: *mut Rect) -> bool;
  fn CGGetActiveDisplayList(max: u32, displays: *mut u32, count: *mut u32) -> i32;
  fn CGDisplayBounds(display: u32) -> Rect;
  fn CGPreflightScreenCaptureAccess() -> bool;
  fn CGWindowListCreateImage(bounds: Rect, option: u32, window: u32, image: u32) -> CfRef;
  fn CGImageGetWidth(image: CfRef) -> usize;
  fn CGImageGetHeight(image: CfRef) -> usize;
  fn CGColorSpaceCreateDeviceRGB() -> CfRef;
  fn CGBitmapContextCreate(
    data: *mut c_void,
    width: usize,
    height: usize,
    bits_per_component: usize,
    bytes_per_row: usize,
    space: CfRef,
    info: u32,
  ) -> CfRef;
  fn CGContextDrawImage(context: CfRef, rect: Rect, image: CfRef);
}

/// Whether macOS lets this process (its responsible app, Orchester) capture other apps' windows. It never prompts.
pub(super) fn screen_recording() -> bool {
  // SAFETY: a query without arguments.
  unsafe { CGPreflightScreenCaptureAccess() }
}

/// A window as the window server lists it.
pub(super) struct Listed {
  pub(super) window_id: u32,
  pub(super) pid: u32,
  pub(super) bounds: Bounds,
  /// Its title, which the window server tells only a process with Screen Recording.
  pub(super) name: Option<String>,
}

/// The normal-layer windows whose process is one of `pids`, front first.
pub(super) fn of_processes(pids: &[u32]) -> Result<Vec<Listed>, String> {
  // SAFETY: a Copy function's answer, owned here.
  let list = unsafe { Cf::owned(CGWindowListCopyWindowInfo(ALL_WINDOWS, 0)) }
    .ok_or_else(|| "the window server lists no windows".to_owned())?;
  let number = |info: &Cf, key: CfRef| {
    // SAFETY: the key is a constant of CoreGraphics, alive for the whole process.
    unsafe { info.get(key) }.and_then(|value| value.number_value())
  };
  let mut found = Vec::new();
  for info in list.items().unwrap_or_default() {
    // SAFETY: reads of CoreGraphics' constant keys, alive for the whole process.
    let (id_key, pid_key, layer_key, bounds_key, name_key) = unsafe {
      (
        kCGWindowNumber,
        kCGWindowOwnerPID,
        kCGWindowLayer,
        kCGWindowBounds,
        kCGWindowName,
      )
    };
    let (Some(id), Some(pid), Some(layer)) = (number(&info, id_key), number(&info, pid_key), number(&info, layer_key))
    else {
      continue;
    };
    let pid = pid as u32;
    if layer != 0.0 || !pids.contains(&pid) {
      continue;
    }
    // SAFETY: a constant key of CoreGraphics.
    let Some(bounds) = unsafe { info.get(bounds_key) }.and_then(|value| rect_of(&value)) else {
      continue;
    };
    if bounds.width <= 0 || bounds.height <= 0 {
      continue;
    }
    found.push(Listed {
      window_id: id as u32,
      pid,
      bounds,
      // SAFETY: a constant key of CoreGraphics.
      name: unsafe { info.get(name_key) }.and_then(|value| value.text()),
    });
  }
  Ok(found)
}

fn rect_of(dictionary: &Cf) -> Option<Bounds> {
  let mut rect = Rect::default();
  // SAFETY: a live dictionary and a writable CGRect.
  unsafe { CGRectMakeWithDictionaryRepresentation(dictionary.raw(), &raw mut rect) }.then(|| bounds(rect))
}

/// `(x, y, width, height)` in whole points.
pub(super) fn bounds_of((x, y, width, height): (f64, f64, f64, f64)) -> Bounds {
  bounds(Rect { x, y, width, height })
}

fn bounds(rect: Rect) -> Bounds {
  // Points of a desktop fit an i32 many times over.
  Bounds {
    x: rect.x.round() as i32,
    y: rect.y.round() as i32,
    width: rect.width.round() as i32,
    height: rect.height.round() as i32,
  }
}

/// The bounds of each active display, in points from the top left of the main one.
pub(super) fn displays() -> Vec<Bounds> {
  let mut ids = [0u32; DISPLAYS];
  let mut count = 0u32;
  // SAFETY: `ids` is writable for DISPLAYS ids, the most the call is told to write; `count` is writable.
  if unsafe { CGGetActiveDisplayList(DISPLAYS as u32, ids.as_mut_ptr(), &raw mut count) } != 0 {
    return Vec::new();
  }
  ids
    .iter()
    .take(count as usize)
    // SAFETY: a display id the system just listed.
    .map(|id| bounds(unsafe { CGDisplayBounds(*id) }))
    .collect()
}

/// The window `window_id` alone, by its id, without its shadow: the window server's image of it, for a macOS
/// without ScreenCaptureKit's screenshots (before 14).
pub(super) fn image_of(window_id: u32) -> Result<NativeCapture, String> {
  // SAFETY: CGRectNull asks for the window's own bounds; the option takes the one window named; the answer is owned.
  let image = unsafe {
    Cf::owned(CGWindowListCreateImage(
      CGRectNull,
      ONE_WINDOW,
      window_id,
      IMAGE_OPTIONS,
    ))
  }
  .ok_or_else(|| "the window server made no image of the window".to_owned())?;
  pixels(&image)
}

/// The pixels of the CGImage `image`, drawn into RGBA memory, opaque.
pub(super) fn pixels(image: &Cf) -> Result<NativeCapture, String> {
  // SAFETY: a live CGImage.
  let (width, height) = unsafe { (CGImageGetWidth(image.raw()), CGImageGetHeight(image.raw())) };
  if width == 0 || height == 0 {
    return Err("the window has no area".to_owned());
  }
  let mut rgba = vec![0u8; width * height * 4];
  // SAFETY: a Create function's answer, owned here.
  let space = unsafe { Cf::owned(CGColorSpaceCreateDeviceRGB()) }.ok_or_else(|| "no RGB color space".to_owned())?;
  // SAFETY: `rgba` is writable for height rows of width × 4 bytes, and outlives the context, dropped below.
  let context = unsafe {
    Cf::owned(CGBitmapContextCreate(
      rgba.as_mut_ptr().cast(),
      width,
      height,
      8,
      width * 4,
      space.raw(),
      RGBA,
    ))
  }
  .ok_or_else(|| "no bitmap to draw the window into".to_owned())?;
  let whole = Rect {
    x: 0.0,
    y: 0.0,
    width: width as f64,
    height: height as f64,
  };
  // SAFETY: a live context and image; the context draws into `rgba` only, its first row the image's top.
  unsafe { CGContextDrawImage(context.raw(), whole, image.raw()) };
  drop(context);
  for pixel in rgba.chunks_exact_mut(4) {
    pixel[3] = u8::MAX;
  }
  Ok(NativeCapture {
    width: u32::try_from(width).map_err(|_| "the window is too wide".to_owned())?,
    height: u32::try_from(height).map_err(|_| "the window is too tall".to_owned())?,
    rgba,
  })
}
