//! How long write transactions hold a database's write lock, and what waits for a lock
//! (`docs/architecture/storage.md`, "Concurrency"). Every connection traces its statements
//! (`sqlite3_trace_v2`). The transaction a `BEGIN` opens is timed from the moment it holds its lock to
//! the end of its `COMMIT` or `ROLLBACK`, and named after its first write (`INSERT agent_events`); a
//! write outside a transaction is timed alone. The checkpoint a commit may run ([`checkpoint`]) comes
//! after the lock is let go, so its time is left out. Each hold is logged at debug level under the
//! target `orchester_store::lock`, one of [`LONG_HOLD`] or longer as a warning, and so is a wait for a
//! lock that lasts, with the statement that waits and its thread. A transaction runs on one thread
//! from its `BEGIN` to its end, which is what ties its statements together.

use std::{
  cell::{Cell, RefCell},
  ffi::c_int,
  time::{Duration, Instant},
};

use rusqlite::{
  Connection,
  hooks::{CheckpointMode, Wal},
  trace::{TraceEvent, TraceEventCodes},
};

/// A write lock held this long or longer is logged as a warning.
const LONG_HOLD: Duration = Duration::from_secs(1);
/// A wait for a lock is logged as a warning once it lasts this long, then every [`WAIT_WARNINGS`].
const FIRST_WAIT_WARNING: Duration = Duration::from_secs(1);
const WAIT_WARNINGS: u64 = 10;
/// How much of the waiting statement's text the warning shows.
const STATEMENT_CHARS: usize = 80;
/// A commit checkpoints the log once it holds this many pages: SQLite's own automatic checkpoint,
/// which the connection's log hook ([`checkpoint`]) takes the place of.
const CHECKPOINT_PAGES: c_int = 1000;

/// A transaction open on this thread.
struct Open {
  /// When it held its lock: its `BEGIN` ended.
  since: Instant,
  /// How long its `BEGIN` waited for the lock.
  waited: Duration,
  /// `BEGIN IMMEDIATE` or `EXCLUSIVE`: it holds the write lock from the start.
  immediate: bool,
  /// Its first write, once it made one.
  label: Option<String>,
}

thread_local! {
  static OPEN: RefCell<Option<Open>> = const { RefCell::new(None) };
  /// The statement running on this thread, which the busy handler names.
  static RUNNING: RefCell<String> = const { RefCell::new(String::new()) };
  /// How long the busy handler paused for the running statement.
  static WAITED: Cell<Duration> = const { Cell::new(Duration::ZERO) };
  /// How long the running statement checkpointed the log after its commit.
  static CHECKPOINTED: Cell<Duration> = const { Cell::new(Duration::ZERO) };
  /// The busy handler gave up on the running statement: a `BEGIN` that ends then opened nothing.
  static GAVE_UP: Cell<bool> = const { Cell::new(false) };
}

/// Starts watching `connection`'s statements and checkpoints.
pub(crate) fn watch(connection: &Connection) {
  connection.trace_v2(
    TraceEventCodes::SQLITE_TRACE_STMT | TraceEventCodes::SQLITE_TRACE_PROFILE,
    Some(traced),
  );
  connection.wal_hook(Some(checkpoint));
}

/// After a commit that left `pages` in the log: SQLite's automatic checkpoint (`PASSIVE`, once the
/// log holds [`CHECKPOINT_PAGES`]), timed. It runs in the committing statement, after the write lock
/// is let go. One that cannot copy the whole log, as readers still read older frames of it, is logged.
fn checkpoint(wal: &Wal, pages: c_int) -> rusqlite::Result<()> {
  if pages < CHECKPOINT_PAGES {
    return Ok(());
  }
  let started = Instant::now();
  match wal.checkpoint_v2(CheckpointMode::PASSIVE) {
    Ok((log, copied)) if copied < log => tracing::debug!(
      target: "orchester_store::lock",
      log,
      copied,
      "a checkpoint left frames that readers still read"
    ),
    Ok(_) => {}
    // The commit stands whatever the checkpoint does, as with SQLite's own: the next commit
    // checkpoints again, so the error is only logged.
    Err(error) => tracing::debug!(target: "orchester_store::lock", %error, "a checkpoint failed"),
  }
  CHECKPOINTED.set(CHECKPOINTED.get() + started.elapsed());
  Ok(())
}

fn traced(event: TraceEvent<'_>) {
  match event {
    // A trigger's program starting is reported as `-- <trigger>`; its statement goes on.
    TraceEvent::Stmt(_, sql) if !sql.trim_start().starts_with("--") => started(sql),
    TraceEvent::Profile(statement, took) => finished(&statement.sql(), took),
    _ => {}
  }
}

/// What a statement does to a transaction.
enum Kind {
  Begin { immediate: bool },
  End,
  Write,
  Other,
}

/// What `sql` does to a transaction, read from its first words; every statement of every connection
/// comes through here, so it allocates nothing.
fn kind(sql: &str) -> Kind {
  let mut words = sql.split_whitespace();
  let first = words.next().unwrap_or_default();
  let is = |keyword: &str| first.eq_ignore_ascii_case(keyword);
  let next_is = |words: &mut std::str::SplitWhitespace<'_>, keywords: &[&str]| {
    words
      .next()
      .is_some_and(|word| keywords.iter().any(|keyword| word.eq_ignore_ascii_case(keyword)))
  };
  if is("BEGIN") {
    Kind::Begin {
      immediate: next_is(&mut words, &["IMMEDIATE", "EXCLUSIVE"]),
    }
  } else if is("COMMIT") || is("END") || (is("ROLLBACK") && !next_is(&mut words, &["TO"])) {
    Kind::End
  } else if ["INSERT", "UPDATE", "DELETE", "REPLACE"].iter().any(|verb| is(verb)) {
    Kind::Write
  } else {
    Kind::Other
  }
}

/// A write's verb and table: `INSERT agent_events`.
fn label(sql: &str) -> String {
  let words: Vec<&str> = sql.split_whitespace().collect();
  let verb = words.first().copied().unwrap_or_default().to_ascii_uppercase();
  let after = |keyword: &str| {
    words
      .iter()
      .position(|word| word.eq_ignore_ascii_case(keyword))
      .and_then(|at| words.get(at + 1))
  };
  let table = match verb.as_str() {
    "UPDATE" => words.iter().skip(1).find(|word| {
      !["OR", "ROLLBACK", "ABORT", "REPLACE", "FAIL", "IGNORE"].contains(&word.to_ascii_uppercase().as_str())
    }),
    "DELETE" => after("FROM"),
    _ => after("INTO"),
  };
  let table = table.map_or("", |table| table.split('(').next().unwrap_or_default());
  format!("{verb} {table}")
}

fn started(sql: &str) {
  WAITED.set(Duration::ZERO);
  CHECKPOINTED.set(Duration::ZERO);
  GAVE_UP.set(false);
  RUNNING.with_borrow_mut(|running| {
    running.clear();
    for word in sql.split_whitespace() {
      if running.len() >= STATEMENT_CHARS {
        break;
      }
      if !running.is_empty() {
        running.push(' ');
      }
      running.push_str(word);
    }
  });
  if matches!(kind(sql), Kind::Write) {
    OPEN.with_borrow_mut(|open| {
      if let Some(open) = open
        && open.label.is_none()
      {
        open.label = Some(label(sql));
      }
    });
  }
}

fn finished(sql: &str, took: Duration) {
  let waited = WAITED.take();
  let checkpointed = CHECKPOINTED.take();
  match kind(sql) {
    Kind::Begin { immediate } if !GAVE_UP.get() => OPEN.set(Some(Open {
      since: Instant::now(),
      waited,
      immediate,
      label: None,
    })),
    Kind::End => {
      if let Some(open) = OPEN.take() {
        ended(open, checkpointed);
      }
    }
    Kind::Write if OPEN.with_borrow(Option::is_none) => {
      held(&label(sql), took.saturating_sub(waited + checkpointed), waited);
    }
    _ => {}
  }
}

/// `open` ended with a statement that checkpointed the log for `checkpointed` after letting go of the lock.
fn ended(open: Open, checkpointed: Duration) {
  let held_for = open.since.elapsed().saturating_sub(checkpointed);
  match (open.immediate, open.label) {
    (true, label) => held(label.as_deref().unwrap_or("nothing written"), held_for, open.waited),
    (false, Some(label)) => {
      tracing::warn!(
        target: "orchester_store::lock",
        transaction = label,
        thread = thread_name(),
        "a deferred transaction wrote: it fails at once (SQLITE_BUSY_SNAPSHOT) when another connection wrote since \
         it began"
      );
      held(&label, held_for, open.waited);
    }
    // A transaction that only read never held the write lock.
    (false, None) => {}
  }
}

/// The write lock was held for `held_for` by `transaction`, after waiting `waited` for it.
fn held(transaction: &str, held_for: Duration, waited: Duration) {
  if held_for >= LONG_HOLD {
    tracing::warn!(
      target: "orchester_store::lock",
      transaction,
      held = ?held_for,
      ?waited,
      thread = thread_name(),
      "a write transaction held the database's write lock long"
    );
  } else {
    tracing::debug!(target: "orchester_store::lock", transaction, held = ?held_for, ?waited, "write lock held");
  }
  #[cfg(feature = "test-support")]
  worst::record(transaction, held_for, waited);
}

/// The busy handler paused `pause` after waiting `before` for the running statement's lock.
pub(crate) fn waited(before: Duration, pause: Duration) {
  WAITED.set(WAITED.get() + pause);
  let after = before + pause;
  let first = before < FIRST_WAIT_WARNING && after >= FIRST_WAIT_WARNING;
  let again = after >= FIRST_WAIT_WARNING && before.as_secs() / WAIT_WARNINGS != after.as_secs() / WAIT_WARNINGS;
  if first || again {
    RUNNING.with_borrow(|statement| {
      tracing::warn!(
        target: "orchester_store::lock",
        statement = statement.as_str(),
        waited = ?after,
        thread = thread_name(),
        "waiting for the database lock"
      );
    });
  }
}

/// The busy handler gave up waiting for the running statement's lock after `waited`.
pub(crate) fn gave_up(waited: Duration) {
  GAVE_UP.set(true);
  RUNNING.with_borrow(|statement| {
    tracing::warn!(
      target: "orchester_store::lock",
      statement = statement.as_str(),
      ?waited,
      thread = thread_name(),
      "gave up waiting for the database lock"
    );
  });
}

fn thread_name() -> String {
  std::thread::current().name().unwrap_or("unnamed").to_owned()
}

/// The write-lock holds of the process, for the contention tests.
#[cfg(feature = "test-support")]
pub mod worst {
  use std::time::Duration;

  use parking_lot::Mutex;

  /// How many of the longest holds are kept.
  const KEPT: usize = 5;

  static HOLDS: Mutex<LockHolds> = Mutex::new(LockHolds {
    count: 0,
    total: Duration::ZERO,
    longest_wait: Duration::ZERO,
    longest: Vec::new(),
  });

  /// The write-lock holds since the process started or [`reset_lock_holds`].
  #[derive(Clone, Debug, PartialEq, Eq)]
  pub struct LockHolds {
    /// How many write transactions held the lock, and for how long together.
    pub count: u64,
    pub total: Duration,
    /// The longest a write waited for the lock.
    pub longest_wait: Duration,
    /// The longest holds, longest first.
    pub longest: Vec<LockHold>,
  }

  /// A write transaction's hold of the write lock.
  #[derive(Clone, Debug, PartialEq, Eq)]
  pub struct LockHold {
    /// Its first write: `INSERT agent_events`.
    pub transaction: String,
    pub held: Duration,
    /// How long it waited for the lock first.
    pub waited: Duration,
  }

  pub(super) fn record(transaction: &str, held: Duration, waited: Duration) {
    let mut holds = HOLDS.lock();
    holds.count += 1;
    holds.total += held;
    holds.longest_wait = holds.longest_wait.max(waited);
    let longest = &mut holds.longest;
    if longest.len() == KEPT && longest.last().is_some_and(|shortest| shortest.held >= held) {
      return;
    }
    longest.push(LockHold {
      transaction: transaction.to_owned(),
      held,
      waited,
    });
    longest.sort_by_key(|hold| std::cmp::Reverse(hold.held));
    longest.truncate(KEPT);
  }

  pub fn lock_holds() -> LockHolds {
    HOLDS.lock().clone()
  }

  pub fn reset_lock_holds() {
    let mut holds = HOLDS.lock();
    holds.count = 0;
    holds.total = Duration::ZERO;
    holds.longest_wait = Duration::ZERO;
    holds.longest.clear();
  }
}
