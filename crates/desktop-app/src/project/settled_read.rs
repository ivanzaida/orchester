//! A read of project data that settles before it is read again: the shape of `ctx.future`, but a change of its deps
//! while it reads does not start a second read beside it. `ctx.future` drops a future whose deps changed and starts
//! another, while the blocking work the dropped one waited on (SQLite, git) runs to its end regardless; when deps move
//! faster than a read takes, no read ever lands and the abandoned ones pile up on the blocking pool.
//!
//! Here at most one read runs per call position. When the deps change while it reads, it finishes, its result is
//! published, and the latest deps are read once more: every change during a read makes one more read together.

use std::{
  future::Future,
  sync::atomic::{AtomicU64, Ordering},
  time::Instant,
};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureState, FutureStatus},
  },
  core::SignalValue,
};

/// The `id` of the next read's trace lines.
static NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// The trace lines of one read (see [`settled_read`]): `read` as it is made, `ended` as it ends or is dropped.
struct Trace {
  name: &'static str,
  id: u64,
  again: bool,
  started: Instant,
  /// Whether its `ended` line is still to be written.
  open: bool,
}

impl Trace {
  fn start(name: &'static str, again: bool) -> Self {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    tracing::trace!(target: crate::app_log::READ_LOG, read = name, id, again);
    Self {
      name,
      id,
      again,
      started: Instant::now(),
      open: true,
    }
  }

  /// Writes the `ended` line of a read that ended, `ok` or failed.
  fn end(mut self, ok: bool) {
    self.open = false;
    let took = self.started.elapsed();
    let (name, id, again) = (self.name, self.id, self.again);
    tracing::trace!(target: crate::app_log::READ_LOG, ended = name, id, ?took, again, ok);
  }
}

impl Drop for Trace {
  /// A read dropped before it ended still ends its pair of lines.
  fn drop(&mut self) {
    if self.open {
      let took = self.started.elapsed();
      let (name, id, again) = (self.name, self.id, self.again);
      tracing::trace!(target: crate::app_log::READ_LOG, ended = name, id, ?took, again, dropped = true);
    }
  }
}

/// The result of a read, with the deps it read.
#[derive(Clone, PartialEq)]
struct Done<D, T> {
  deps: D,
  value: T,
}

impl<D, T> DevtoolsInspectable for Done<D, T> {}

/// The error of a read, with the deps it read.
#[derive(Clone, PartialEq)]
struct Failed<D, E> {
  deps: D,
  error: E,
}

impl<D, E> DevtoolsInspectable for Failed<D, E> {}

/// Reads `deps` with `read` at this call position of the render, as `ctx.future` would, but never while a read of
/// the position runs (see the module). The state is `ctx.future`'s: `Pending` while a read runs, `data` the last value
/// that was read (kept while a read runs and after one failed), `error` the latest read's error while it is
/// `Rejected`. The render that sees a read end gets its outcome, even when it starts the next read.
///
/// `name` names the read in two trace lines of [`crate::app_log::READ_LOG`] per read: `read` as it starts, on the
/// render's thread as the other reads of the log are, with whether deps that moved after an earlier read made it
/// (`again`); `ended` as it ends, on whichever thread ran it, with its duration (`took`), `again` and whether it
/// succeeded (`ok`), or `dropped` when it was dropped before it ended (its component went). Both carry the read's
/// `id`, unique in the process, which pairs them.
pub fn settled_read<D, T, E, F, Fut>(ctx: &mut Ctx, name: &'static str, deps: D, read: F) -> FutureState<T, E>
where
  D: Clone + PartialEq + Send + Sync + 'static,
  T: SignalValue + Clone + PartialEq + Send + Sync + 'static,
  E: SignalValue + Clone + PartialEq + Send + Sync + 'static,
  F: Fn(D) -> Fut + Send + Sync + 'static,
  Fut: Future<Output = Result<T, E>> + Send + 'static,
{
  settled_read_traced(ctx, (name, |_| true), deps, read)
}

/// [`settled_read`] whose trace lines name only the reads of deps `traced` tells: the others read nothing (say, there
/// is nothing to read yet) and have no line.
pub fn settled_read_traced<D, T, E, F, Fut>(
  ctx: &mut Ctx,
  (name, traced): (&'static str, fn(&D) -> bool),
  deps: D,
  read: F,
) -> FutureState<T, E>
where
  D: Clone + PartialEq + Send + Sync + 'static,
  T: SignalValue + Clone + PartialEq + Send + Sync + 'static,
  E: SignalValue + Clone + PartialEq + Send + Sync + 'static,
  F: Fn(D) -> Fut + Send + Sync + 'static,
  Fut: Future<Output = Result<T, E>> + Send + 'static,
{
  let action = ctx.future_action(move |(deps, again): (D, bool)| {
    let trace = traced(&deps).then(|| Trace::start(name, again));
    let reading = read(deps.clone());
    async move {
      let result = reading.await;
      if let Some(trace) = trace {
        trace.end(result.is_ok());
      }
      result
        .map(|value| Done {
          deps: deps.clone(),
          value,
        })
        .map_err(|error| Failed { deps, error })
    }
  });
  let state = action.state().get();
  if !action.is_active() {
    let last = match state.status {
      FutureStatus::Fulfilled => state.data.as_ref().map(|done| &done.deps),
      FutureStatus::Rejected => state.error.as_ref().map(|failed| &failed.deps),
      FutureStatus::Idle | FutureStatus::Pending => None,
    };
    if last != Some(&deps) {
      let again = !state.is_idle();
      action.run_if_idle((deps, again));
    }
  }
  let shown = if state.is_idle() {
    action.state().get_untracked()
  } else {
    state
  };
  FutureState {
    status: shown.status,
    data: shown.data.map(|done| done.value),
    error: shown.error.map(|failed| failed.error),
  }
}
