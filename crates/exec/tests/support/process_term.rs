#![allow(unsafe_code)]
//! Installs SIGTERM handling only in a copied macOS stand-in. The handler does no allocation or I/O.

use std::{
  fs, io,
  sync::atomic::{AtomicBool, Ordering},
  thread,
  time::Duration,
};

static CLOSED: AtomicBool = AtomicBool::new(false);

extern "C" fn close(_signal: i32) {
  CLOSED.store(true, Ordering::Relaxed);
}

pub fn wait_for_close(before_close: impl FnOnce() -> io::Result<()>) -> io::Result<()> {
  // SAFETY: this process is a stand-in; the static handler remains valid and only sets an atomic flag.
  if unsafe { libc::signal(libc::SIGTERM, close as *const () as libc::sighandler_t) } == libc::SIG_ERR {
    return Err(io::Error::last_os_error());
  }
  fs::write(std::env::var_os("ORCHESTER_PROCESS_READY").expect("ready"), "ready")?;
  while !CLOSED.load(Ordering::Relaxed) {
    thread::sleep(Duration::from_millis(5));
  }
  before_close()
}
