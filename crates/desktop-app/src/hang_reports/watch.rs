use std::time::Duration;

use sentry::Hub;

use super::{
  Heartbeat, event,
  stack::{UiStack, UiThread},
};

/// How long the window's thread may give no sign of life before the window counts as not responding.
pub const THRESHOLD: Duration = Duration::from_secs(10);

/// A pause between two checks this long means the watchdog itself did not run: the computer slept or the OS suspended
/// the process. The window's thread did not run either, so the time is not counted against it.
pub const ASLEEP: Duration = Duration::from_secs(5);

/// What a check sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Noted {
  /// The window's thread gave no sign of life for `quiet`: the *Application not responding* report went out.
  NotResponding { quiet: Duration },
  /// The window's thread, reported not responding, lives again after `hung` without a sign of life.
  Responding { hung: Duration },
}

/// Watches the window's thread's [`Heartbeat`]: one report when it has given no sign of life for [`THRESHOLD`] with
/// the window not minimized, carrying the thread's stack, and a second when it lives again. Nothing is read or sent
/// while the thread's hub has no client (error reports are off, a debug build, the tests); a window that never beat
/// yet is not watched.
pub struct HangWatch {
  heartbeat: Heartbeat,
  ui: UiThread,
  /// The clock's time at the last check.
  checked: Option<Duration>,
  /// When the watchdog last woke from a sleep of the computer or the process ([`ASLEEP`]).
  woke: Duration,
  /// The last beat before the hang that was reported.
  hang: Option<Duration>,
}

impl HangWatch {
  pub fn new(heartbeat: Heartbeat, ui: UiThread) -> Self {
    Self {
      heartbeat,
      ui,
      checked: None,
      woke: Duration::ZERO,
      hang: None,
    }
  }

  /// Checks the heartbeat now and reports what changed through the current thread's hub.
  pub fn check(&mut self) -> Option<Noted> {
    let now = self.heartbeat.now();
    if self
      .checked
      .is_some_and(|checked| now.saturating_sub(checked) >= ASLEEP)
    {
      self.woke = now;
    }
    self.checked = Some(now);
    let beat = self.heartbeat.last()?;
    match self.hang {
      Some(before) if beat > before => {
        self.hang = None;
        Some(self.responding(beat - before))
      }
      Some(_) => None,
      None => self.not_responding(beat, now.saturating_sub(beat.max(self.woke))),
    }
  }

  /// Reports the hang since the `beat` when the thread has been `quiet` long enough, unless the person turned the
  /// error reports off: then the window's thread is not even suspended to read its stack.
  fn not_responding(&mut self, beat: Duration, quiet: Duration) -> Option<Noted> {
    if quiet < THRESHOLD || self.heartbeat.minimized() || crate::error_reports::turned_off() {
      return None;
    }
    let hub = Hub::current();
    let client = hub.client().filter(|client| client.is_enabled())?;
    let screen = self.heartbeat.screen();
    let stack = self.ui.stack();
    let captured = matches!(stack, UiStack::Captured(_));
    tracing::warn!(
      seconds = quiet.as_secs(),
      screen,
      captured,
      "the window is not responding; reported"
    );
    hub.capture_event(event::not_responding(quiet, screen, stack));
    // Sent now: a person who gives up on the window ends the process before the client's queue would have.
    client.flush(Some(orchester_report::client::FLUSH_TIMEOUT));
    self.hang = Some(beat);
    Some(Noted::NotResponding { quiet })
  }

  fn responding(&self, hung: Duration) -> Noted {
    let screen = self.heartbeat.screen();
    tracing::warn!(seconds = hung.as_secs(), screen, "the window responds again");
    Hub::current().capture_event(event::responding(hung, screen));
    Noted::Responding { hung }
  }
}
