#![allow(unsafe_code)]
//! A small native window for the native UI driver's tests (`tests/native_ui.rs`): a status label, a text field, a
//! Save button and a checkbox, Win32's own controls, which UI Automation sees through their standard providers. Save
//! puts `Saved: <the field's text>` in the label and prints `saved <text>`. It prints `window ready` once its window
//! shows, without activating it — on the desktop, or beyond it with the argument `off-screen` — and keeps running
//! until it is closed or ended. The window's own creation and message loop have no safe wrapper, hence this file's
//! `unsafe`; Windows only.

#[cfg(not(windows))]
#[allow(clippy::print_stderr, reason = "a test program says why it cannot run")]
fn main() {
  eprintln!("the native fixture window is Windows only");
  std::process::exit(1);
}

#[cfg(windows)]
#[allow(clippy::print_stderr, reason = "a test program says why it failed")]
fn main() {
  if let Err(error) = fixture::run() {
    eprintln!("the native fixture window failed: {error}");
    std::process::exit(1);
  }
}

#[cfg(windows)]
mod fixture {
  use std::{
    io::{self, Write},
    mem, ptr,
  };

  use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    Graphics::Gdi::CreateSolidBrush,
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::{
      BN_CLICKED, BS_AUTOCHECKBOX, BS_PUSHBUTTON, CreateWindowExW, DefWindowProcW, DispatchMessageW, ES_AUTOHSCROLL,
      GetDlgItem, GetMessageW, GetWindowTextW, MSG, PostQuitMessage, RegisterClassW, SW_SHOWNOACTIVATE, SetWindowTextW,
      ShowWindow, TranslateMessage, WM_COMMAND, WM_DESTROY, WNDCLASSW, WS_BORDER, WS_CHILD, WS_OVERLAPPEDWINDOW,
      WS_TABSTOP, WS_VISIBLE,
    },
  };

  /// The window's title.
  const TITLE: &str = "Orchester native fixture";
  /// Its outer size, which the tests compare a screenshot's with.
  pub const SIZE: (i32, i32) = (420, 260);
  /// The colour of its background, `0x00BBGGRR`: the tests look for it in a screenshot.
  const BACKGROUND: u32 = 0x003D_8A24;
  const STATUS: i32 = 1;
  const FIELD: i32 = 2;
  const SAVE: i32 = 3;
  const REMEMBER: i32 = 4;

  fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
  }

  pub fn run() -> io::Result<()> {
    let class = wide("OrchesterNativeFixture");
    // SAFETY: no module name: this program's own module.
    let instance = unsafe { GetModuleHandleW(ptr::null()) };
    // SAFETY: WNDCLASSW is a C record whose zero values are valid; the fields set below make the class.
    let mut definition: WNDCLASSW = unsafe { mem::zeroed() };
    definition.lpfnWndProc = Some(procedure);
    definition.hInstance = instance;
    definition.lpszClassName = class.as_ptr();
    // SAFETY: a solid brush the class keeps for the program's lifetime.
    definition.hbrBackground = unsafe { CreateSolidBrush(BACKGROUND) };
    // SAFETY: the record and its class name outlive the registration.
    if unsafe { RegisterClassW(&definition) } == 0 {
      return Err(io::Error::last_os_error());
    }
    let (x, y) = if std::env::args().any(|argument| argument == "off-screen") {
      (-20_000, -20_000)
    } else {
      (120, 120)
    };
    let window = create(
      &class,
      TITLE,
      WS_OVERLAPPEDWINDOW,
      (x, y, SIZE.0, SIZE.1),
      (ptr::null_mut(), 0),
    )?;
    let child = WS_CHILD | WS_VISIBLE | WS_TABSTOP;
    create(
      &wide("STATIC"),
      "Status: idle",
      WS_CHILD | WS_VISIBLE,
      (16, 16, 380, 20),
      (window, STATUS),
    )?;
    create(
      &wide("EDIT"),
      "",
      child | WS_BORDER | ES_AUTOHSCROLL as u32,
      (16, 48, 250, 24),
      (window, FIELD),
    )?;
    create(
      &wide("BUTTON"),
      "Save",
      child | BS_PUSHBUTTON as u32,
      (280, 48, 100, 26),
      (window, SAVE),
    )?;
    create(
      &wide("BUTTON"),
      "Remember me",
      child | BS_AUTOCHECKBOX as u32,
      (16, 86, 200, 22),
      (window, REMEMBER),
    )?;
    // SAFETY: the window was created above; it shows without taking the focus from the person's window.
    unsafe { ShowWindow(window, SW_SHOWNOACTIVATE) };
    say("window ready")?;
    // SAFETY: MSG is a C record whose zero values are valid.
    let mut message: MSG = unsafe { mem::zeroed() };
    // SAFETY: message is writable; a null window takes every message of this thread.
    while unsafe { GetMessageW(&mut message, ptr::null_mut(), 0, 0) } > 0 {
      // SAFETY: the message came from this thread's queue.
      unsafe {
        TranslateMessage(&message);
        DispatchMessageW(&message);
      }
    }
    Ok(())
  }

  /// A window of `class` showing `text`, at `(x, y, width, height)` in `parent` with the id `id`, or a top-level one.
  fn create(
    class: &[u16],
    text: &str,
    style: u32,
    (x, y, width, height): (i32, i32, i32, i32),
    (parent, id): (HWND, i32),
  ) -> io::Result<HWND> {
    let text = wide(text);
    // SAFETY: terminated class and text; a child's menu handle is its id, as Win32 takes it.
    let window = unsafe {
      CreateWindowExW(
        0,
        class.as_ptr(),
        text.as_ptr(),
        style,
        x,
        y,
        width,
        height,
        parent,
        id as isize as _,
        GetModuleHandleW(ptr::null()),
        ptr::null(),
      )
    };
    if window.is_null() {
      return Err(io::Error::last_os_error());
    }
    Ok(window)
  }

  fn say(line: &str) -> io::Result<()> {
    let mut out = io::stdout().lock();
    writeln!(out, "{line}")?;
    out.flush()
  }

  /// The text of the child `id` of `window`.
  fn text_of(window: HWND, id: i32) -> String {
    let mut buffer = [0u16; 512];
    // SAFETY: the buffer is writable for the length given.
    let length = unsafe { GetWindowTextW(GetDlgItem(window, id), buffer.as_mut_ptr(), buffer.len() as i32) };
    String::from_utf16_lossy(&buffer[..usize::try_from(length).unwrap_or(0)])
  }

  unsafe extern "system" fn procedure(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
      WM_COMMAND if (wparam & 0xffff) as i32 == SAVE && ((wparam >> 16) & 0xffff) as u32 == BN_CLICKED => {
        let text = text_of(window, FIELD);
        let status = wide(&format!("Saved: {text}"));
        // SAFETY: the label is a child of this window; the text is terminated.
        unsafe { SetWindowTextW(GetDlgItem(window, STATUS), status.as_ptr()) };
        if say(&format!("saved {text}")).is_err() {
          // SAFETY: ends the loop of a fixture whose output is gone.
          unsafe { PostQuitMessage(1) };
        }
        0
      }
      WM_DESTROY => {
        // SAFETY: ends this thread's message loop.
        unsafe { PostQuitMessage(0) };
        0
      }
      // SAFETY: the default handling of a message this window does not handle itself.
      _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
  }
}
