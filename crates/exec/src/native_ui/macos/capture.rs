#![allow(unsafe_code)]
//! ScreenCaptureKit has no bindings in the build, so it is loaded at run time and reached through the Objective-C
//! runtime, which is FFI without a safe wrapper: one window's image by its window id, as a desktop-independent window
//! (its own content, wherever it lies and whatever covers it; never the screen around it). Its screenshots came with
//! macOS 14; before, or where the framework is missing, the window server's image of that one window is taken
//! instead ([`window::image_of`]).

use std::{
  ffi::{CStr, c_void},
  sync::{OnceLock, mpsc},
  time::Duration,
};

use block2::RcBlock;
use objc2::{
  msg_send,
  rc::{Allocated, Retained},
  runtime::{AnyClass, AnyObject, Bool},
};
use objc2_foundation::NSString;

use super::{
  super::{NativeCapture, NativeWindow},
  cf::Cf,
  window,
};

/// The longest a capture is waited for.
const TIMEOUT: Duration = Duration::from_secs(10);

/// ScreenCaptureKit's library. It came with macOS 12.3 while the app starts on macOS 11, so it is never linked
/// (the loader would refuse to start the app without it) but loaded when a capture first needs it.
const FRAMEWORK: &CStr = c"/System/Library/Frameworks/ScreenCaptureKit.framework/ScreenCaptureKit";

/// Whether ScreenCaptureKit is loaded; loads it once, and keeps it loaded for the life of the process.
fn framework_loaded() -> bool {
  static LOADED: OnceLock<bool> = OnceLock::new();
  // SAFETY: `FRAMEWORK` is a NUL-terminated path; a null answer only says the library is missing, and a loaded one
  // is never closed, so its classes stay valid.
  *LOADED.get_or_init(|| !unsafe { libc::dlopen(FRAMEWORK.as_ptr(), libc::RTLD_LAZY) }.is_null())
}

type Answer = Result<NativeCapture, String>;

/// The window `target` alone, in pixels.
pub(super) fn window_image(target: &NativeWindow) -> Answer {
  if !framework_loaded() {
    return window::image_of(target.window_id);
  }
  let (Some(content), Some(filter), Some(configuration), Some(manager)) = (
    AnyClass::get(c"SCShareableContent"),
    AnyClass::get(c"SCContentFilter"),
    AnyClass::get(c"SCStreamConfiguration"),
    AnyClass::get(c"SCScreenshotManager"),
  ) else {
    return window::image_of(target.window_id);
  };
  let classes = Classes {
    filter,
    configuration,
    manager,
  };
  let (sender, answers) = mpsc::channel::<Answer>();
  let (window_id, bounds) = (target.window_id, target.bounds);
  let listed = RcBlock::new(move |shareable: *mut AnyObject, error: *mut AnyObject| {
    let answer = if shareable.is_null() {
      Err(described(error, "ScreenCaptureKit lists no windows"))
    } else {
      // SAFETY: ScreenCaptureKit hands a live SCShareableContent for the length of this call.
      let shareable = unsafe { &*shareable };
      classes.capture(shareable, (window_id, bounds.width, bounds.height), &sender)
    };
    if let Err(failed) = answer {
      // The receiver is gone only once the capture was given up on; nobody is left to tell.
      drop(sender.send(Err(failed)));
    }
  });
  // SAFETY: the class method of SCShareableContent with its BOOL arguments and a block of (content, error), which
  // it copies and calls once.
  let () = unsafe {
    msg_send![
      content,
      getShareableContentExcludingDesktopWindows: Bool::YES,
      onScreenWindowsOnly: Bool::NO,
      completionHandler: &*listed
    ]
  };
  answers
    .recv_timeout(TIMEOUT)
    .map_err(|_| format!("ScreenCaptureKit gave no image within {} s", TIMEOUT.as_secs()))?
}

/// The classes a capture uses after the window list.
#[derive(Clone, Copy)]
struct Classes {
  filter: &'static AnyClass,
  configuration: &'static AnyClass,
  manager: &'static AnyClass,
}

impl Classes {
  /// Starts the capture of the window `window_id`, `width` × `height` points, of `shareable`; its image goes to
  /// `sender`.
  fn capture(
    self,
    shareable: &AnyObject,
    (window_id, width, height): (u32, i32, i32),
    sender: &mpsc::Sender<Answer>,
  ) -> Result<(), String> {
    let window = find(shareable, window_id).ok_or_else(|| "ScreenCaptureKit does not list the window".to_owned())?;
    // SAFETY: SCContentFilter's initializer for one window, on a fresh allocation; SCStreamConfiguration's `new`;
    // their properties take and answer the C types given (float, size_t, BOOL).
    let (filter, configuration) = unsafe {
      let allocated: Allocated<AnyObject> = msg_send![self.filter, alloc];
      let filter: Retained<AnyObject> = msg_send![allocated, initWithDesktopIndependentWindow: &*window];
      let scale: f32 = msg_send![&*filter, pointPixelScale];
      let configuration: Retained<AnyObject> = msg_send![self.configuration, new];
      let pixels = |points: i32| (f64::from(points.max(1)) * f64::from(scale)).round() as usize;
      let () = msg_send![&*configuration, setWidth: pixels(width)];
      let () = msg_send![&*configuration, setHeight: pixels(height)];
      let () = msg_send![&*configuration, setShowsCursor: Bool::NO];
      (filter, configuration)
    };
    let sender = sender.clone();
    let taken = RcBlock::new(move |image: *mut c_void, error: *mut AnyObject| {
      // SAFETY: ScreenCaptureKit hands a CGImage, or null, alive for the length of this call; it is retained here.
      let answer = unsafe { Cf::retained(image.cast_const()) }
        .ok_or_else(|| described(error, "ScreenCaptureKit made no image of the window"))
        .and_then(|image| window::pixels(&image));
      // The receiver is gone only once the capture was given up on; nobody is left to tell.
      drop(sender.send(answer));
    });
    // SAFETY: SCScreenshotManager's class method with a live filter and configuration and a block of (image, error),
    // which it copies and calls once.
    let () = unsafe {
      msg_send![
        self.manager,
        captureImageWithFilter: &*filter,
        configuration: &*configuration,
        completionHandler: &*taken
      ]
    };
    Ok(())
  }
}

/// The SCWindow of `shareable` whose id is `window_id`.
fn find(shareable: &AnyObject, window_id: u32) -> Option<Retained<AnyObject>> {
  // SAFETY: `windows` is an NSArray of SCWindow; `count`, `objectAtIndex:` within it and `windowID` (a CGWindowID)
  // are its documented messages.
  unsafe {
    let windows: Retained<AnyObject> = msg_send![shareable, windows];
    let count: usize = msg_send![&*windows, count];
    (0..count).find_map(|index| {
      let window: Retained<AnyObject> = msg_send![&*windows, objectAtIndex: index];
      let id: u32 = msg_send![&*window, windowID];
      (id == window_id).then_some(window)
    })
  }
}

/// What the NSError `error` says, or `otherwise` when there is none.
fn described(error: *mut AnyObject, otherwise: &str) -> String {
  if error.is_null() {
    return otherwise.to_owned();
  }
  // SAFETY: ScreenCaptureKit hands a live NSError for the length of the call; its description is an NSString.
  let text: Option<Retained<NSString>> = unsafe { msg_send![&*error, localizedDescription] };
  text.map_or_else(|| otherwise.to_owned(), |text| format!("{otherwise}: {text}"))
}
