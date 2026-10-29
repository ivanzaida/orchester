//! Reports of a frozen window (ORC-356): a window's thread that gives no sign of life for ten seconds is reported once,
//! scrubbed, with its stack (Windows x86-64) or a field that says it was not captured, and its recovery is reported
//! too; nothing is read or sent while error reports are off, a minimized window and a sleeping computer are not
//! counted, and the stack of a blocked thread is read from another thread, which resumes it.

#[path = "support/blocked_thread.rs"]
mod blocked_thread;

use std::{
  sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
  },
  time::Duration,
};

use blocked_thread::BlockedThread;
use orchester_app::{
  error_reports::{self, Environment, RELEASE},
  hang_reports::{
    self, BEAT, Clock, HangWatch, Heartbeat, Noted, THRESHOLD,
    event::{CAPTURED, NOT_RESPONDING, SCREEN_TAG, STACK_TAG},
    stack::{UiStack, UiThread},
  },
};
use sentry::{Hub, Level, protocol::Event, test::with_captured_events_options};

/// A clock the test moves by hand.
#[derive(Default)]
struct FakeClock(AtomicU64);

impl FakeClock {
  fn advance(&self, by: Duration) {
    let millis = u64::try_from(by.as_millis()).expect("a test's step fits in 64 bits");
    self.0.fetch_add(millis, Ordering::SeqCst);
  }
}

impl Clock for FakeClock {
  fn now(&self) -> Duration {
    Duration::from_millis(self.0.load(Ordering::SeqCst))
  }
}

/// A watch on a blocked stand-in for the window's thread, with its heartbeat on a fake clock.
struct Watched {
  clock: Arc<FakeClock>,
  heartbeat: Heartbeat,
  watch: HangWatch,
  /// What each check sent, at the clock's time.
  noted: Vec<(Duration, Noted)>,
}

impl Watched {
  fn new(window: &BlockedThread) -> Self {
    let clock = Arc::new(FakeClock::default());
    let heartbeat = Heartbeat::new(clock.clone());
    let watch = HangWatch::new(heartbeat.clone(), window.thread());
    Self {
      clock,
      heartbeat,
      watch,
      noted: Vec::new(),
    }
  }

  /// `seconds` of checks a second apart, as the watchdog makes them.
  fn checks(&mut self, seconds: u64) {
    for _ in 0..seconds {
      self.clock.advance(BEAT);
      if let Some(noted) = self.watch.check() {
        self.noted.push((self.clock.now(), noted));
      }
    }
  }
}

fn seconds(seconds: u64) -> Duration {
  Duration::from_secs(seconds)
}

fn reported(body: impl FnOnce()) -> Vec<Event<'static>> {
  with_captured_events_options(body, error_reports::options(Environment::Qa))
}

#[test]
fn hang_reports_a_stuck_window_is_reported_once_with_its_stack_and_its_recovery_noted() {
  let window = BlockedThread::start();
  let mut watched = Watched::new(&window);
  watched.heartbeat.beat("tasks");

  let events = reported(|| {
    watched.checks(25);
    watched.heartbeat.beat("task");
    watched.checks(1);
  });
  window.release();

  assert_eq!(
    watched.noted,
    [
      (THRESHOLD, Noted::NotResponding { quiet: THRESHOLD }),
      (seconds(26), Noted::Responding { hung: seconds(25) }),
    ]
  );
  assert_eq!(events.len(), 2, "{events:?}");
  assert_not_responding(&events[0], "tasks");
  let recovered = &events[1];
  assert_eq!(recovered.level, Level::Info);
  assert_eq!(
    recovered.message.as_deref(),
    Some("the window responds again after 25 s")
  );
  assert_eq!(recovered.tags.get(SCREEN_TAG).map(String::as_str), Some("task"));
}

fn assert_not_responding(event: &Event<'static>, screen: &str) {
  assert_eq!(event.level, Level::Error);
  assert_eq!(event.release.as_deref(), Some(RELEASE));
  assert_eq!(event.environment.as_deref(), Some("qa"));
  assert_eq!(event.tags.get("process").map(String::as_str), Some("app"));
  assert_eq!(event.tags.get(SCREEN_TAG).map(String::as_str), Some(screen));
  assert!(event.breadcrumbs.values.is_empty() && event.extra.is_empty() && event.user.is_none());
  assert!(!event.debug_meta.images.is_empty(), "the loaded modules: {event:?}");
  let exception = &event.exception.values[0];
  assert_eq!(exception.ty, NOT_RESPONDING);
  let value = exception
    .value
    .as_deref()
    .expect("how long the window has not responded");
  assert!(value.starts_with("the window has not responded for 10 s"), "{value}");
  let stack = event.tags.get(STACK_TAG).map(String::as_str);
  if cfg!(all(windows, target_arch = "x86_64")) {
    assert_eq!(stack, Some(CAPTURED));
    let frames = &exception
      .stacktrace
      .as_ref()
      .expect("the window's thread's stack")
      .frames;
    assert!(frames.len() >= 2, "{frames:?}");
    assert!(
      frames
        .iter()
        .all(|frame| frame.instruction_addr.is_some() && frame.function.is_none()),
      "addresses only: {frames:?}"
    );
  } else {
    let expected = if cfg!(target_os = "macos") {
      "not captured on macOS yet"
    } else {
      "not captured on this platform yet"
    };
    assert_eq!(stack, Some(expected));
    assert!(exception.stacktrace.is_none() && value.ends_with(expected), "{value}");
  }
}

#[test]
fn hang_reports_a_window_that_hangs_again_after_recovering_is_reported_again() {
  let window = BlockedThread::start();
  let mut watched = Watched::new(&window);
  watched.heartbeat.beat("tasks");

  let events = reported(|| {
    watched.checks(12);
    watched.heartbeat.beat("overview");
    watched.checks(12);
  });
  window.release();

  assert_eq!(
    watched.noted,
    [
      (THRESHOLD, Noted::NotResponding { quiet: THRESHOLD }),
      (seconds(13), Noted::Responding { hung: seconds(12) }),
      (seconds(22), Noted::NotResponding { quiet: THRESHOLD }),
    ]
  );
  assert_eq!(events.len(), 3, "{events:?}");
  assert_not_responding(&events[2], "overview");
}

#[test]
fn hang_reports_nothing_is_read_or_sent_while_error_reports_are_off() {
  let window = BlockedThread::start();
  let mut watched = Watched::new(&window);
  watched.heartbeat.beat("tasks");

  watched.checks(30);
  window.release();

  assert!(watched.noted.is_empty(), "{:?}", watched.noted);
  assert!(Hub::current().client().is_none() && Hub::main().client().is_none());
  assert!(
    hang_reports::heartbeat().is_none(),
    "no watchdog runs in the tests: the app starts one only with its error reports"
  );
}

#[test]
fn hang_reports_a_minimized_window_and_a_sleeping_computer_are_not_counted() {
  let window = BlockedThread::start();
  let mut watched = Watched::new(&window);

  let events = reported(|| {
    watched.checks(15);
    watched.heartbeat.beat("tasks");
    watched.heartbeat.set_minimized(true);
    watched.checks(15);
    watched.heartbeat.set_minimized(false);
    watched.heartbeat.beat("tasks");
    // The computer sleeps for a minute: the watchdog does not run either.
    watched.clock.advance(seconds(60));
    watched.checks(9);
  });
  window.release();

  assert!(watched.noted.is_empty(), "{:?}", watched.noted);
  assert!(events.is_empty(), "{events:?}");
}

#[test]
fn hang_reports_the_stack_of_a_blocked_thread_is_read_and_the_thread_resumed() {
  let window = BlockedThread::start();

  let first = window.thread().stack();
  let second = window.thread().stack();
  let own = UiThread::current().stack();
  // It ends only if each read resumed it.
  window.release();

  if cfg!(all(windows, target_arch = "x86_64")) {
    for stack in [first, second] {
      let UiStack::Captured(frames) = stack else {
        panic!("the blocked thread's stack is read: {stack:?}");
      };
      assert!(
        frames.len() >= 3,
        "the wait, the helper and the thread's start: {frames:?}"
      );
      assert!(frames.iter().all(|&address| address != 0), "{frames:?}");
    }
    assert_eq!(own, UiStack::NotCaptured("the thread cannot read its own stack"));
  } else {
    assert!(matches!(first, UiStack::NotCaptured(_)), "{first:?}");
  }
}
