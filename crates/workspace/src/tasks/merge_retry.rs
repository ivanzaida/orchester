//! Orchester trying a merge again by itself (`crates/desktop-app/design/orchester.pen`, board `Review · redesign`, 09j
//! `GhnHQ`). A merge from the task page, `merge_run` or the merge queue that did not land for a cause that passes by
//! itself ([`IntegrationProblem::passes`]) is tried again up to [`RETRIES`] times, [`delay_before`] each, and each try
//! is recorded as an attempt by *Orchester · retry N of 3* before the run shows Not merged. While it waits, *Try now*
//! ([`try_now`]) ends the wait at once and *Stop retrying* ([`stop_retrying`]) ends the merge as the last attempt left
//! it; while a try goes on the run still shows it ([`waiting`], due since the try began), and *Stop retrying* then ends
//! the merge once that try did not land. A lasting cause is not tried again. Blocks while it waits; callers run it off
//! the UI thread.

use std::{
  collections::HashMap,
  num::NonZeroU32,
  sync::{Arc, LazyLock},
  time::{Duration, Instant},
};

use orchester_core::{
  id::RunId,
  integration::{
    Delivering,
    attempt::MergeAsker,
    retry::{RETRIES, delay_before},
  },
  revision::Revision,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use parking_lot::{Condvar, Mutex};

use super::{
  integration::{IntegrationProblem, Merged, merge_in_as},
  merge_attempts::Asked,
  worktree_removal::WorktreeRemover,
};

/// The runs whose merge waits to be tried again, in every project.
static WAITING: LazyLock<Mutex<HashMap<RunId, Arc<Wait>>>> = LazyLock::new(Default::default);

/// Orchester's next try of a run's merge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetryWait {
  /// Which try it is, from 1 to [`RETRIES`].
  pub number: NonZeroU32,
  /// When it tries, unless a person asks for it now.
  pub at: Timestamp,
}

/// What a person asked of a wait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Request {
  Now,
  Stop,
}

struct Wait {
  next: RetryWait,
  asked: Mutex<Option<Request>>,
  told: Condvar,
  /// When the try began, once the wait ended and Orchester tries.
  began: Mutex<Option<Timestamp>>,
}

/// Merges run `run` whose run worktrees are `worktrees` as [`merge_in_as`] does, `asked` asking, and tries it again by
/// itself while it does not land for a cause that passes. How the last try went.
pub fn merge_retrying(
  worktrees: &WorktreeRemover,
  programs: &Programs,
  (run, expected, delivering): (RunId, Option<&Revision>, Option<Delivering<'_>>),
  asked: Asked,
) -> Result<Merged, IntegrationProblem> {
  let first = merge_in_as(worktrees, programs, (run, expected, delivering), asked);
  retried(worktrees, programs, (run, expected, delivering), asked, first)
}

/// Orchester's tries of the merge [`merge_retrying`] makes after `first`, the try `asked` asked for, while it does not
/// land for a cause that passes. How the last try went.
pub fn retried(
  worktrees: &WorktreeRemover,
  programs: &Programs,
  (run, expected, delivering): (RunId, Option<&Revision>, Option<Delivering<'_>>),
  asked: Asked,
  first: Result<Merged, IntegrationProblem>,
) -> Result<Merged, IntegrationProblem> {
  let mut merged = first;
  for number in (1..=RETRIES).filter_map(NonZeroU32::new) {
    match &merged {
      Err(problem) if problem.passes() => {}
      _ => break,
    }
    if !wait(run, number) {
      tracing::info!(%run, "a person stopped Orchester's retries of the merge");
      break;
    }
    tracing::info!(%run, retry = number.get(), "Orchester tries the merge again");
    let retry = (MergeAsker::Retry { number }, asked.1);
    merged = merge_in_as(worktrees, programs, (run, expected, delivering), retry);
    if tried(run) {
      tracing::info!(%run, "a person stopped Orchester's retries of the merge while it tried");
      break;
    }
  }
  merged
}

/// Waits for try `number` of `run`'s merge: [`delay_before`] it, or until a person asks for it now. Whether to try,
/// which a person who stopped the retries said not to; a try stays shown until [`tried`].
fn wait(run: RunId, number: NonZeroU32) -> bool {
  let delay = delay_before(number);
  let waiting = Arc::new(Wait {
    next: RetryWait {
      number,
      at: Timestamp::from_unix_millis(Timestamp::now().unix_millis() + millis(delay)),
    },
    asked: Mutex::new(None),
    told: Condvar::new(),
    began: Mutex::new(None),
  });
  WAITING.lock().insert(run, waiting.clone());
  let deadline = Instant::now() + delay;
  let mut asked = waiting.asked.lock();
  while asked.is_none() && !waiting.told.wait_until(&mut asked, deadline).timed_out() {}
  if *asked == Some(Request::Stop) {
    drop(asked);
    WAITING.lock().remove(&run);
    return false;
  }
  // What was asked of the wait is done; what is asked from now on is asked of the try.
  *asked = None;
  *waiting.began.lock() = Some(Timestamp::now());
  true
}

/// Ends the try of `run`'s merge that [`wait`] began. Whether a person stopped the retries meanwhile.
fn tried(run: RunId) -> bool {
  WAITING
    .lock()
    .remove(&run)
    .is_some_and(|waiting| *waiting.asked.lock() == Some(Request::Stop))
}

fn millis(delay: Duration) -> i64 {
  i64::try_from(delay.as_millis()).unwrap_or(i64::MAX)
}

/// *Try now*: ends the wait for Orchester's next try of `run`'s merge, which begins at once. Whether it waited or
/// tried.
pub fn try_now(run: RunId) -> bool {
  tell(run, Request::Now)
}

/// *Stop retrying*: ends Orchester's retries of `run`'s merge; the run stays Not merged as its last attempt left it, or
/// as the try going on leaves it. Whether it waited or tried.
pub fn stop_retrying(run: RunId) -> bool {
  tell(run, Request::Stop)
}

/// Orchester's next try of `run`'s merge, while it waits for it; while the try goes on, due when it began.
pub fn waiting(run: RunId) -> Option<RetryWait> {
  WAITING.lock().get(&run).map(|waiting| match *waiting.began.lock() {
    Some(at) => RetryWait {
      number: waiting.next.number,
      at,
    },
    None => waiting.next,
  })
}

fn tell(run: RunId, what: Request) -> bool {
  let Some(waiting) = WAITING.lock().get(&run).cloned() else {
    return false;
  };
  let mut asked = waiting.asked.lock();
  // Stop outweighs Now: asked during a try, it is the one that still has something to end.
  if *asked != Some(Request::Stop) {
    *asked = Some(what);
  }
  drop(asked);
  waiting.told.notify_all();
  true
}
