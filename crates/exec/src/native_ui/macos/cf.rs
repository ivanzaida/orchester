#![allow(unsafe_code)]
//! CoreFoundation, in which Accessibility and the window server answer, has no safe wrapper in the build: an owned
//! reference ([`Cf`]) that is released once, and the reads the driver needs of strings, numbers, booleans, arrays and
//! dictionaries. Every read checks the object's type first, so a value of another type reads as none.

use std::ffi::{c_char, c_void};

/// A CoreFoundation object, of any type.
pub(super) type CfRef = *const c_void;

/// `kCFStringEncodingUTF8`.
const UTF8: u32 = 0x0800_0100;
/// `kCFNumberFloat64Type`.
const FLOAT64: isize = 6;

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
  static kCFBooleanTrue: CfRef;
  fn CFRelease(object: CfRef);
  fn CFRetain(object: CfRef) -> CfRef;
  fn CFGetTypeID(object: CfRef) -> usize;
  fn CFHash(object: CfRef) -> usize;
  fn CFStringGetTypeID() -> usize;
  fn CFNumberGetTypeID() -> usize;
  fn CFBooleanGetTypeID() -> usize;
  fn CFArrayGetTypeID() -> usize;
  fn CFDictionaryGetTypeID() -> usize;
  fn CFStringCreateWithBytes(allocator: CfRef, bytes: *const u8, length: isize, encoding: u32, external: u8) -> CfRef;
  fn CFStringGetLength(string: CfRef) -> isize;
  fn CFStringGetMaximumSizeForEncoding(length: isize, encoding: u32) -> isize;
  fn CFStringGetCString(string: CfRef, buffer: *mut c_char, size: isize, encoding: u32) -> u8;
  fn CFNumberCreate(allocator: CfRef, kind: isize, value: *const c_void) -> CfRef;
  fn CFNumberGetValue(number: CfRef, kind: isize, value: *mut c_void) -> u8;
  fn CFBooleanGetValue(boolean: CfRef) -> u8;
  fn CFArrayGetCount(array: CfRef) -> isize;
  fn CFArrayGetValueAtIndex(array: CfRef, index: isize) -> CfRef;
  fn CFDictionaryGetValue(dictionary: CfRef, key: CfRef) -> CfRef;
}

/// A CoreFoundation object this holds one reference to, released when it drops.
pub(super) struct Cf(CfRef);

impl Cf {
  /// Takes over the reference a `Create` or `Copy` function answered; `None` for null.
  ///
  /// # Safety
  /// `raw` is null or a CoreFoundation object whose reference the caller owns and hands over.
  pub(super) unsafe fn owned(raw: CfRef) -> Option<Self> {
    (!raw.is_null()).then_some(Self(raw))
  }

  /// A reference of its own to an object a `Get` function answered, which its holder keeps; `None` for null.
  ///
  /// # Safety
  /// `raw` is null or a CoreFoundation object alive for this call.
  pub(super) unsafe fn retained(raw: CfRef) -> Option<Self> {
    if raw.is_null() {
      return None;
    }
    // SAFETY: a live object, as the caller states; the reference taken is released by `Drop`.
    Some(Self(unsafe { CFRetain(raw) }))
  }

  /// The string `text`.
  pub(super) fn string(text: &str) -> Result<Self, String> {
    let length = isize::try_from(text.len()).map_err(|_| "the text is too long".to_owned())?;
    // SAFETY: the bytes are valid UTF-8 of `length` bytes, copied by the call (not external).
    let raw = unsafe { CFStringCreateWithBytes(std::ptr::null(), text.as_ptr(), length, UTF8, 0) };
    // SAFETY: a Create function's answer, owned here.
    unsafe { Self::owned(raw) }.ok_or_else(|| "CoreFoundation made no string".to_owned())
  }

  /// The number `value`.
  pub(super) fn number(value: f64) -> Result<Self, String> {
    // SAFETY: the value is a readable f64, the type the kind names; the call copies it.
    let raw = unsafe { CFNumberCreate(std::ptr::null(), FLOAT64, (&raw const value).cast()) };
    // SAFETY: a Create function's answer, owned here.
    unsafe { Self::owned(raw) }.ok_or_else(|| "CoreFoundation made no number".to_owned())
  }

  /// `kCFBooleanTrue`.
  pub(super) fn yes() -> Option<Self> {
    // SAFETY: a constant of CoreFoundation, alive for the whole process.
    unsafe { Self::retained(kCFBooleanTrue) }
  }

  pub(super) fn raw(&self) -> CfRef {
    self.0
  }

  /// Its `CFHash`, which follows its equality.
  pub(super) fn hash(&self) -> u64 {
    // SAFETY: a live object held here.
    unsafe { CFHash(self.0) as u64 }
  }

  fn is(&self, type_id: unsafe extern "C" fn() -> usize) -> bool {
    // SAFETY: a type id function without arguments, and a live object held here.
    unsafe { CFGetTypeID(self.0) == type_id() }
  }

  /// Its text, when it is a string.
  pub(super) fn text(&self) -> Option<String> {
    if !self.is(CFStringGetTypeID) {
      return None;
    }
    // SAFETY: a live string.
    let length = unsafe { CFStringGetLength(self.0) };
    // SAFETY: a plain computation of the bytes `length` UTF-16 units take at most, with the terminator added.
    let size = unsafe { CFStringGetMaximumSizeForEncoding(length, UTF8) }.checked_add(1)?;
    let mut buffer = vec![0u8; usize::try_from(size).ok()?];
    // SAFETY: the buffer is writable for `size` bytes, which the call is given.
    if unsafe { CFStringGetCString(self.0, buffer.as_mut_ptr().cast(), size, UTF8) } == 0 {
      return None;
    }
    let end = buffer.iter().position(|byte| *byte == 0).unwrap_or(buffer.len());
    buffer.truncate(end);
    String::from_utf8(buffer).ok()
  }

  /// Its value, when it is a number (any kind, read as a float) or a boolean (0 or 1).
  pub(super) fn number_value(&self) -> Option<f64> {
    if self.is(CFBooleanGetTypeID) {
      return self.flag().map(f64::from);
    }
    if !self.is(CFNumberGetTypeID) {
      return None;
    }
    let mut value = 0f64;
    // SAFETY: a live number, read into a writable f64, the type the kind names (converted, maybe lossily).
    unsafe { CFNumberGetValue(self.0, FLOAT64, (&raw mut value).cast()) };
    Some(value)
  }

  /// Its value, when it is a boolean.
  pub(super) fn flag(&self) -> Option<bool> {
    // SAFETY: a live boolean, checked first.
    self
      .is(CFBooleanGetTypeID)
      .then(|| unsafe { CFBooleanGetValue(self.0) } != 0)
  }

  /// Its items, when it is an array, each with a reference of its own.
  pub(super) fn items(&self) -> Option<Vec<Self>> {
    if !self.is(CFArrayGetTypeID) {
      return None;
    }
    // SAFETY: a live array.
    let count = unsafe { CFArrayGetCount(self.0) };
    Some(
      (0..count)
        // SAFETY: an index below the array's count; the array keeps the item alive while it is retained.
        .filter_map(|index| unsafe { Self::retained(CFArrayGetValueAtIndex(self.0, index)) })
        .collect(),
    )
  }

  /// The value of `key`, when it is a dictionary that holds it.
  ///
  /// # Safety
  /// `key` is a live CoreFoundation object, such as a constant key of a framework.
  pub(super) unsafe fn get(&self, key: CfRef) -> Option<Self> {
    if !self.is(CFDictionaryGetTypeID) {
      return None;
    }
    // SAFETY: a live dictionary and key; the dictionary keeps the value alive while it is retained.
    unsafe { Self::retained(CFDictionaryGetValue(self.0, key)) }
  }
}

impl Drop for Cf {
  fn drop(&mut self) {
    // SAFETY: this holds one reference, released once.
    unsafe { CFRelease(self.0) }
  }
}
