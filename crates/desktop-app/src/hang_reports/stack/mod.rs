//! The stack of the window's thread, read from another thread while it is stuck: on Windows x86-64 the thread is
//! suspended for the moment its context is read and unwound (`windows`); elsewhere it is not captured yet, and the
//! report says so (`unsupported`).

#[cfg(all(windows, target_arch = "x86_64"))]
mod windows;
#[cfg(all(windows, target_arch = "x86_64"))]
use windows as platform;

#[cfg(not(all(windows, target_arch = "x86_64")))]
mod unsupported;
#[cfg(not(all(windows, target_arch = "x86_64")))]
use unsupported as platform;

/// The most frames a captured stack keeps, innermost first; deeper callers are left out.
pub const MAX_FRAMES: usize = 128;

/// A thread whose stack another thread can read: the window's, taken on it with [`UiThread::current`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UiThread(platform::ThreadId);

/// What reading a thread's stack found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiStack {
  /// The instruction addresses of its frames, innermost first: the address the thread stood at, then each caller's
  /// return address. GlitchTip resolves them to functions, files and lines in the uploaded debug files.
  Captured(Vec<u64>),
  /// Why there is no stack, as the report says it.
  NotCaptured(&'static str),
}

impl UiThread {
  /// The thread that calls this.
  pub fn current() -> Self {
    Self(platform::current())
  }

  /// The thread's stack now. Never call it on the thread itself: a thread cannot be stopped to read its own stack, and
  /// that is answered as not captured.
  pub fn stack(self) -> UiStack {
    platform::stack(self.0)
  }
}
