#![allow(unsafe_code)]
//! A hidden window belonging only to the spawned stand-in, to prove WM_CLOSE through the process tool.

use std::{fs, io, ptr, thread, time::Duration};

use windows_sys::Win32::UI::WindowsAndMessaging::{
  CreateWindowExW, DispatchMessageW, IsWindow, MSG, PM_REMOVE, PeekMessageW,
};

pub fn wait_for_close() -> io::Result<()> {
  let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
  // SAFETY: terminated class name; the built-in STATIC class needs no registration or instance;
  // no parent, menu or creation parameter. The window is hidden (no WS_VISIBLE).
  let window = unsafe {
    CreateWindowExW(
      0,
      class.as_ptr(),
      ptr::null(),
      0,
      0,
      0,
      1,
      1,
      ptr::null_mut(),
      ptr::null_mut(),
      ptr::null_mut(),
      ptr::null(),
    )
  };
  if window.is_null() {
    return Err(io::Error::last_os_error());
  }
  let ready = std::env::var_os("ORCHESTER_PROCESS_READY").expect("test's ready file");
  fs::write(ready, "ready")?;
  // SAFETY: MSG is a C record whose zero values are valid.
  let mut message: MSG = unsafe { std::mem::zeroed() };
  // SAFETY: the handle names our stand-in's window; IsWindow only inspects it.
  while unsafe { IsWindow(window) } != 0 {
    // SAFETY: message is writable; null HWND polls this thread's messages without blocking.
    while unsafe { PeekMessageW(&mut message, ptr::null_mut(), 0, 0, PM_REMOVE) } != 0 {
      // SAFETY: this message was produced by this thread's queue.
      unsafe { DispatchMessageW(&message) };
    }
    thread::sleep(Duration::from_millis(5));
  }
  Ok(())
}
