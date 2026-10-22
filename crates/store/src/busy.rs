//! Surviving contention for the databases (`docs/architecture/storage.md`, "Concurrency"). Every
//! connection waits for a lock with [`wait_for_lock`], its busy handler: it tries again with growing
//! pauses until the lock is free or [`BUDGET_MS`] has passed, so a writer that waits behind the runs,
//! the app and the Orchestrator chat waits instead of failing. A write transaction takes the write lock
//! when it begins, before it reads, so that is the only place it meets a busy lock. [`retry`] runs a
//! whole transaction again when a busy error still reaches it: one SQLite answers without calling the
//! handler (`SQLITE_BUSY_SNAPSHOT`, a read upgraded to a write), within the same budget. Past the
//! budget the error is [`StoreError::Unavailable`], which says how long it waited and for which code.

use std::{
  sync::atomic::{AtomicU64, Ordering},
  thread,
  time::{Duration, Instant},
};

use rusqlite::ffi;

use crate::{RunStoreError, StoreError, lock_watch};

/// How long a connection waits for a lock in total, and a transaction run again is retried, before the
/// database counts as unavailable, in milliseconds: a minute.
const BUDGET_MS: u64 = 60_000;
/// The busy handler's pauses between attempts to take a lock, in milliseconds; the last one repeats.
/// Short pauses keep a waiting writer from losing every turn to the writers that come after it.
const PAUSES_MS: [u64; 8] = [1, 2, 5, 10, 15, 20, 25, 50];
/// The first pause before a transaction runs again; each next one doubles, up to [`LONGEST_RETRY_PAUSE`].
const FIRST_RETRY_PAUSE: Duration = Duration::from_millis(10);
const LONGEST_RETRY_PAUSE: Duration = Duration::from_secs(1);

/// [`BUDGET_MS`], or the shorter budget a test set.
static BUDGET: AtomicU64 = AtomicU64::new(BUDGET_MS);

fn budget() -> Duration {
  Duration::from_millis(BUDGET.load(Ordering::Relaxed))
}

/// Makes every connection of the process give up waiting for a lock after `budget` instead of a
/// minute, so a test can hold a lock past it.
#[cfg(feature = "test-support")]
pub fn set_lock_budget_for_test(budget: Duration) {
  let millis = u64::try_from(budget.as_millis()).expect("a test's budget fits in u64 milliseconds");
  BUDGET.store(millis, Ordering::Relaxed);
}

/// The busy handler of every connection: `count` attempts to take the lock failed so far. Waits
/// before the next attempt, or gives up once the pauses add up to the budget.
pub(crate) fn wait_for_lock(count: i32) -> bool {
  let count = usize::try_from(count).unwrap_or_default();
  let waited = slept_before(count);
  if waited >= budget() {
    lock_watch::gave_up(waited);
    return false;
  }
  let pause = Duration::from_millis(PAUSES_MS[count.min(PAUSES_MS.len() - 1)]);
  thread::sleep(pause);
  lock_watch::waited(waited, pause);
  true
}

/// How long the handler slept before attempt `count`.
fn slept_before(count: usize) -> Duration {
  let listed = count.min(PAUSES_MS.len());
  let repeated = u64::try_from(count - listed).unwrap_or(u64::MAX);
  let last = PAUSES_MS[PAUSES_MS.len() - 1];
  Duration::from_millis(
    PAUSES_MS[..listed]
      .iter()
      .sum::<u64>()
      .saturating_add(repeated.saturating_mul(last)),
  )
}

/// The name of SQLite's extended result code `code` when it says the database or a table is locked:
/// what the log and [`StoreError`] name.
pub(crate) const fn code_name(code: i32) -> Option<&'static str> {
  match code {
    ffi::SQLITE_BUSY => Some("SQLITE_BUSY"),
    ffi::SQLITE_BUSY_RECOVERY => Some("SQLITE_BUSY_RECOVERY"),
    ffi::SQLITE_BUSY_SNAPSHOT => Some("SQLITE_BUSY_SNAPSHOT"),
    ffi::SQLITE_BUSY_TIMEOUT => Some("SQLITE_BUSY_TIMEOUT"),
    ffi::SQLITE_LOCKED => Some("SQLITE_LOCKED"),
    ffi::SQLITE_LOCKED_SHAREDCACHE => Some("SQLITE_LOCKED_SHAREDCACHE"),
    ffi::SQLITE_LOCKED_VTAB => Some("SQLITE_LOCKED_VTAB"),
    _ => None,
  }
}

/// The extended code of `error` when another connection holds the database: worth trying again.
/// `SQLITE_LOCKED` is a conflict within one connection, which waiting does not end.
fn transient_code(error: &rusqlite::Error) -> Option<i32> {
  match error {
    rusqlite::Error::SqliteFailure(failure, _) if failure.code == rusqlite::ErrorCode::DatabaseBusy => {
      Some(failure.extended_code)
    }
    _ => None,
  }
}

/// An error that may say another connection holds the database.
pub(crate) trait Contention: Sized {
  /// The extended code when it does.
  fn busy_code(&self) -> Option<i32>;

  /// The error once the database stayed busy for `waited`.
  fn unavailable(self, waited: Duration) -> Self;
}

impl Contention for StoreError {
  fn busy_code(&self) -> Option<i32> {
    match self {
      Self::Sqlite(error) => transient_code(error),
      _ => None,
    }
  }

  fn unavailable(self, waited: Duration) -> Self {
    match self {
      Self::Sqlite(source) => Self::Unavailable { waited, source },
      other => other,
    }
  }
}

impl Contention for RunStoreError {
  fn busy_code(&self) -> Option<i32> {
    match self {
      Self::Store(error) => error.busy_code(),
      _ => None,
    }
  }

  fn unavailable(self, waited: Duration) -> Self {
    match self {
      Self::Store(error) => Self::Store(error.unavailable(waited)),
      other => other,
    }
  }
}

/// Runs `attempt`, a whole transaction that rolls back when it fails, again while it fails because
/// the database is busy, with growing pauses, until [`BUDGET`] has passed since the first attempt.
/// Each retry is logged as a warning naming `what` and the code. Other errors come back at once.
pub(crate) fn retry<T, E: Contention>(what: &'static str, mut attempt: impl FnMut() -> Result<T, E>) -> Result<T, E> {
  let started = Instant::now();
  let mut pause = FIRST_RETRY_PAUSE;
  loop {
    let error = match attempt() {
      Ok(value) => return Ok(value),
      Err(error) => error,
    };
    let Some(code) = error.busy_code() else {
      return Err(error);
    };
    let code = code_name(code).unwrap_or("SQLITE_BUSY");
    let waited = started.elapsed();
    if waited + pause > budget() {
      tracing::error!(what, code, ?waited, "the database stayed busy; the write gives up");
      return Err(error.unavailable(waited));
    }
    tracing::warn!(what, code, ?waited, "the database is busy; the write starts again");
    thread::sleep(pause);
    pause = (pause * 2).min(LONGEST_RETRY_PAUSE);
  }
}

#[cfg(feature = "test-support")]
thread_local! {
  static INJECTED: std::cell::RefCell<Vec<i32>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Makes the next transactions the store runs again on busy errors ([`retry`]) on this thread fail
/// with `codes`, one each, after they began: as when another connection committed in between.
#[cfg(feature = "test-support")]
pub fn fail_next_writes_for_test(codes: &[i32]) {
  INJECTED.with_borrow_mut(|injected| {
    injected.clear();
    injected.extend(codes.iter().rev());
  });
}

/// The error a test injected for this attempt, if any.
#[cfg(feature = "test-support")]
pub(crate) fn injected_failure() -> Result<(), rusqlite::Error> {
  match INJECTED.with_borrow_mut(Vec::pop) {
    Some(code) => Err(rusqlite::Error::SqliteFailure(
      ffi::Error::new(code),
      Some("injected by the test".to_owned()),
    )),
    None => Ok(()),
  }
}
