#![allow(unsafe_code)]
//! The Accessibility API of ApplicationServices has no safe wrapper in the build: an app's and its elements'
//! attributes, their actions and settable attributes, the window server id of a window element, and whether macOS
//! trusts this process with it. Nothing here sends input: actions are the element's own AX actions.

use std::ffi::c_void;

use super::{
  super::accessibility::AxValue,
  cf::{Cf, CfRef},
};

/// `kAXErrorInvalidUIElement`: the element went away.
const GONE: i32 = -25_202;
/// `kAXErrorCannotComplete`: the app did not answer in time.
const CANNOT_COMPLETE: i32 = -25_204;
/// `kAXErrorAttributeUnsupported`, `kAXErrorNoValue`, `kAXErrorNotImplemented`: there is nothing to read.
const NOTHING: [i32; 3] = [-25_205, -25_212, -25_208];
/// `kAXErrorAPIDisabled`: macOS does not trust this process with Accessibility.
const DISABLED: i32 = -25_211;
/// Not an `AXError`: CoreFoundation made no object to pass, which AX never answers (its codes are negative).
const UNMADE: i32 = 1;
/// `kAXValueCGPointType` and `kAXValueCGSizeType`.
const POINT: u32 = 1;
const SIZE: u32 = 2;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
  fn AXIsProcessTrusted() -> u8;
  fn AXUIElementCreateApplication(pid: i32) -> CfRef;
  fn AXUIElementCopyAttributeValue(element: CfRef, attribute: CfRef, value: *mut CfRef) -> i32;
  fn AXUIElementSetAttributeValue(element: CfRef, attribute: CfRef, value: CfRef) -> i32;
  fn AXUIElementIsAttributeSettable(element: CfRef, attribute: CfRef, settable: *mut u8) -> i32;
  fn AXUIElementCopyActionNames(element: CfRef, names: *mut CfRef) -> i32;
  fn AXUIElementPerformAction(element: CfRef, action: CfRef) -> i32;
  fn AXUIElementSetMessagingTimeout(element: CfRef, seconds: f32) -> i32;
  fn AXValueCreate(kind: u32, value: *const c_void) -> CfRef;
  fn AXValueGetValue(value: CfRef, kind: u32, out: *mut c_void) -> u8;
  /// Not in the public headers, but the only link from an AX window to its window server id; every macOS since 10.6
  /// has it, and window managers rely on it.
  fn _AXUIElementGetWindow(element: CfRef, window: *mut u32) -> i32;
}

/// Whether macOS trusts this process (its responsible app, Orchester) with Accessibility. It never prompts.
pub(super) fn trusted() -> bool {
  // SAFETY: a query without arguments.
  unsafe { AXIsProcessTrusted() != 0 }
}

/// Why an AX call failed, by its `AXError`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct AxError(i32);

impl AxError {
  const UNMADE: Self = Self(UNMADE);

  /// Whether the app did not answer in time.
  pub(super) fn busy(self) -> bool {
    self.0 == CANNOT_COMPLETE
  }
}

impl std::fmt::Display for AxError {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self.0 {
      GONE => formatter.write_str("the element went away"),
      CANNOT_COMPLETE => formatter.write_str("the app did not answer"),
      DISABLED => formatter.write_str("macOS does not trust Orchester with Accessibility"),
      UNMADE => formatter.write_str("CoreFoundation made no object to pass"),
      code => write!(formatter, "AXError {code}"),
    }
  }
}

fn checked(code: i32) -> Result<(), AxError> {
  if code == 0 { Ok(()) } else { Err(AxError(code)) }
}

/// An element of an app's Accessibility tree: the app itself, a window, or anything inside one.
pub(super) struct AxElement(Cf);

impl AxElement {
  /// The app of the process `pid`.
  pub(super) fn application(pid: u32) -> Option<Self> {
    let pid = i32::try_from(pid).ok()?;
    // SAFETY: a Create function, which answers an element owned here (or null).
    unsafe { Cf::owned(AXUIElementCreateApplication(pid)) }.map(Self)
  }

  /// Its `CFHash`, which its ref is made of.
  pub(super) fn hash(&self) -> u64 {
    self.0.hash()
  }

  /// The value of `attribute`; `None` when it has none.
  fn copy(&self, attribute: &str) -> Result<Option<Cf>, AxError> {
    let name = Cf::string(attribute).map_err(|_| AxError::UNMADE)?;
    let mut value: CfRef = std::ptr::null();
    // SAFETY: a live element and attribute name; `value` is writable and receives a reference owned by the caller.
    let code = unsafe { AXUIElementCopyAttributeValue(self.0.raw(), name.raw(), &raw mut value) };
    if NOTHING.contains(&code) {
      return Ok(None);
    }
    checked(code)?;
    // SAFETY: a Copy function's answer, owned here.
    Ok(unsafe { Cf::owned(value) })
  }

  pub(super) fn text(&self, attribute: &str) -> Result<Option<String>, AxError> {
    Ok(self.copy(attribute)?.and_then(|value| value.text()))
  }

  pub(super) fn flag(&self, attribute: &str) -> Result<Option<bool>, AxError> {
    Ok(self.copy(attribute)?.and_then(|value| value.flag()))
  }

  /// Its `AXValue`: text, or a number (a boolean reads as 0 or 1).
  pub(super) fn value(&self) -> Result<Option<AxValue>, AxError> {
    Ok(self.copy("AXValue")?.and_then(|value| {
      value
        .text()
        .map(AxValue::Text)
        .or_else(|| value.number_value().map(AxValue::Number))
    }))
  }

  /// The elements `attribute` lists, such as `AXChildren` or `AXWindows`.
  pub(super) fn elements(&self, attribute: &str) -> Result<Vec<Self>, AxError> {
    Ok(
      self
        .copy(attribute)?
        .and_then(|value| value.items())
        .map(|items| items.into_iter().map(Self).collect())
        .unwrap_or_default(),
    )
  }

  /// Whether `attribute` may be set.
  pub(super) fn settable(&self, attribute: &str) -> Result<bool, AxError> {
    let name = Cf::string(attribute).map_err(|_| AxError::UNMADE)?;
    let mut settable = 0u8;
    // SAFETY: a live element and attribute name; `settable` is a writable Boolean.
    let code = unsafe { AXUIElementIsAttributeSettable(self.0.raw(), name.raw(), &raw mut settable) };
    if NOTHING.contains(&code) {
      return Ok(false);
    }
    checked(code)?;
    Ok(settable != 0)
  }

  /// The names of the actions it performs.
  pub(super) fn actions(&self) -> Result<Vec<String>, AxError> {
    let mut names: CfRef = std::ptr::null();
    // SAFETY: a live element; `names` is writable and receives an array owned by the caller.
    let code = unsafe { AXUIElementCopyActionNames(self.0.raw(), &raw mut names) };
    if NOTHING.contains(&code) {
      return Ok(Vec::new());
    }
    checked(code)?;
    // SAFETY: a Copy function's answer, owned here.
    let names = unsafe { Cf::owned(names) };
    Ok(
      names
        .and_then(|names| names.items())
        .map(|items| items.iter().filter_map(Cf::text).collect())
        .unwrap_or_default(),
    )
  }

  /// Performs its AX action `action`.
  pub(super) fn perform(&self, action: &str) -> Result<(), AxError> {
    let name = Cf::string(action).map_err(|_| AxError::UNMADE)?;
    // SAFETY: a live element and action name.
    checked(unsafe { AXUIElementPerformAction(self.0.raw(), name.raw()) })
  }

  fn set(&self, attribute: &str, value: &Cf) -> Result<(), AxError> {
    let name = Cf::string(attribute).map_err(|_| AxError::UNMADE)?;
    // SAFETY: a live element, attribute name and value; the call does not keep the value past it unless it retains it.
    checked(unsafe { AXUIElementSetAttributeValue(self.0.raw(), name.raw(), value.raw()) })
  }

  /// Sets its `AXValue`.
  pub(super) fn set_value(&self, value: &AxValue) -> Result<(), AxError> {
    let value = match value {
      AxValue::Text(text) => Cf::string(text),
      AxValue::Number(number) => Cf::number(*number),
    }
    .map_err(|_| AxError::UNMADE)?;
    self.set("AXValue", &value)
  }

  /// Sets the boolean `attribute`, such as `AXSelected`, to true.
  pub(super) fn set_true(&self, attribute: &str) -> Result<(), AxError> {
    let yes = Cf::yes().ok_or(AxError::UNMADE)?;
    self.set(attribute, &yes)
  }

  /// Its `AXPosition` and `AXSize`, in points from the top left of the main display: `(x, y, width, height)`.
  pub(super) fn frame(&self) -> Result<Option<(f64, f64, f64, f64)>, AxError> {
    let (Some(position), Some(size)) = (self.copy("AXPosition")?, self.copy("AXSize")?) else {
      return Ok(None);
    };
    let (mut point, mut extent) = ([0f64; 2], [0f64; 2]);
    // SAFETY: live AXValues, read as the CGPoint and CGSize (two f64 each) their kinds name, into writable arrays.
    let read = unsafe {
      AXValueGetValue(position.raw(), POINT, point.as_mut_ptr().cast()) != 0
        && AXValueGetValue(size.raw(), SIZE, extent.as_mut_ptr().cast()) != 0
    };
    Ok(read.then_some((point[0], point[1], extent[0], extent[1])))
  }

  /// Moves it, a window, to `(x, y)` in points; the app may keep it nearer the displays.
  pub(super) fn move_to(&self, x: f64, y: f64) -> Result<(), AxError> {
    let point = [x, y];
    // SAFETY: a readable CGPoint (two f64), copied by the call; the answer is owned here.
    let value = unsafe { Cf::owned(AXValueCreate(POINT, point.as_ptr().cast())) }.ok_or(AxError::UNMADE)?;
    self.set("AXPosition", &value)
  }

  /// Its window server id, when it is a window.
  pub(super) fn window_id(&self) -> Option<u32> {
    let mut id = 0u32;
    // SAFETY: a live element; `id` is a writable CGWindowID.
    (unsafe { _AXUIElementGetWindow(self.0.raw(), &raw mut id) } == 0).then_some(id)
  }

  /// Makes calls to its app give up after `seconds` instead of the system's default.
  pub(super) fn answer_within(&self, seconds: f32) -> Result<(), AxError> {
    // SAFETY: a live element; the timeout applies to calls made through it.
    checked(unsafe { AXUIElementSetMessagingTimeout(self.0.raw(), seconds) })
  }
}
