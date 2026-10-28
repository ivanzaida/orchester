//! Reads whether Windows plays animation effects: `SystemParametersInfoW` has no safe wrapper, so `unsafe` is allowed
//! here and only here.
#![allow(unsafe_code)]

use windows_sys::{
  Win32::UI::WindowsAndMessaging::{SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW},
  core::BOOL,
};

use super::MotionError;

/// Whether *Animation effects* is on (Settings › Accessibility › Visual effects).
pub fn animations_on() -> Result<bool, MotionError> {
  let mut on: BOOL = 0;
  // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL through `pvparam`, which points at `on`, a live BOOL that
  // outlives the call; `uiparam` and `fwinini` are unused for this action and 0.
  let read = unsafe { SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, (&raw mut on).cast(), 0) };
  if read == 0 {
    return Err(MotionError::Unreadable);
  }
  Ok(on != 0)
}
