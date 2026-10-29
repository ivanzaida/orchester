//! Reports of a frozen window (`docs/error-reports.md`, "A frozen window"): a hang is no panic, so without these
//! nothing would tell GlitchTip the window stopped responding.
//!
//! The window's thread beats once a second from a lurq interval, which its event loop runs between events
//! ([`beat::Beat`]); a watchdog thread ([`Watchdog`]) checks the [`Heartbeat`] every second ([`HangWatch`]). Ten
//! seconds without a beat send one *Application not responding* report with the window's thread's stack, read from
//! the watchdog thread ([`stack`]); the next beat sends a second report, how long the window hung. A minimized
//! window is not watched: the heartbeat carries the window's minimized state as the window's thread last saw it.
//! Nor is the time the computer slept or the OS suspended the process counted ([`watch::ASLEEP`]).
//!
//! The watchdog runs only while the app's error reports do (`error_reports::start`): only the installed app or a
//! release build a QA harness started, with the person's setting on; never a debug build, a QA copy or the tests. It
//! starts before the window opens and stops once it closed, so the work after the window (the runners stopping) is
//! not watched; the window is watched from its first beat on.

pub mod beat;
pub mod event;
mod heartbeat;
pub mod stack;
mod watch;

use std::{
  sync::{Arc, OnceLock, mpsc},
  thread::JoinHandle,
  time::Duration,
};

use self::stack::UiThread;
pub use self::{
  heartbeat::{Clock, Heartbeat, SystemClock},
  watch::{ASLEEP, HangWatch, Noted, THRESHOLD},
};

/// How often the window's thread beats and the watchdog checks.
pub const BEAT: Duration = Duration::from_secs(1);

/// The heartbeat of the window the running watchdog watches; unset while none runs.
static HEARTBEAT: OnceLock<Heartbeat> = OnceLock::new();

/// The heartbeat the window beats, when a watchdog watches it.
pub fn heartbeat() -> Option<Heartbeat> {
  HEARTBEAT.get().cloned()
}

/// The thread that watches the window's thread; it stops when dropped.
pub struct Watchdog {
  /// Dropped to stop the thread.
  stop: Option<mpsc::Sender<()>>,
  thread: Option<JoinHandle<()>>,
}

impl Watchdog {
  /// Watches the calling thread, which runs the window's event loop, from a thread of its own; reports go through the
  /// app's client, which [`crate::error_reports::start`] started. `None`, logged, when the thread cannot start or a
  /// watchdog started before.
  pub fn start() -> Option<Self> {
    let heartbeat = Heartbeat::new(Arc::new(SystemClock::new()));
    if HEARTBEAT.set(heartbeat.clone()).is_err() {
      tracing::error!("a watchdog watches the window already");
      return None;
    }
    let mut watch = HangWatch::new(heartbeat, UiThread::current());
    let (stop, stopped) = mpsc::channel::<()>();
    let thread = std::thread::Builder::new()
      .name("hang-watchdog".to_owned())
      .spawn(move || {
        while let Err(mpsc::RecvTimeoutError::Timeout) = stopped.recv_timeout(BEAT) {
          watch.check();
        }
      });
    match thread {
      Ok(thread) => Some(Self {
        stop: Some(stop),
        thread: Some(thread),
      }),
      Err(error) => {
        tracing::error!(%error, "cannot start the watchdog of the window; a frozen window is not reported");
        None
      }
    }
  }
}

impl Drop for Watchdog {
  fn drop(&mut self) {
    drop(self.stop.take());
    if let Some(thread) = self.thread.take()
      && thread.join().is_err()
    {
      tracing::error!("the watchdog of the window panicked");
    }
  }
}
