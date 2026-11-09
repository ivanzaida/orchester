//! While the run's merge job works, Orchester may wait between its tries (`merge_retry`) without the job's state
//! changing: a component of its own looks at that wait once a second, and reads the page again when it begins, changes
//! or ends, so the status turns to *Retrying by itself* and back and its attempt stays current. Nothing renders but
//! this component, and only while the job works. The attempt page mounts it.

use std::time::Duration;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Interval},
  },
  components::Column,
  core::Signal,
  node::Element,
};
use orchester_core::id::RunId;
use orchester_workspace::tasks::merge_retry::{self, RetryWait};
use parking_lot::Mutex;

const TICK: Duration = Duration::from_secs(1);

#[derive(Clone)]
struct Props {
  run: RunId,
  /// Counts what makes the page be read again.
  again: Signal<u64>,
}

impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.run == other.run
  }
}

impl DevtoolsInspectable for Props {}

/// The watch of `run`'s wait, which bumps `again` when it changes.
pub(super) fn watch(ctx: &mut Ctx, run: RunId, again: &Signal<u64>) -> Element {
  ctx.mount_keyed::<Watch>(
    &format!("merge-attempt-watch-{run}"),
    Props {
      run,
      again: again.clone(),
    },
  )
}

struct Watch {
  tick: Signal<u64>,
  /// The wait the watch saw last; not a signal, as only a change of it reads the page again.
  seen: Mutex<Option<RetryWait>>,
  interval: Interval,
}

impl Component for Watch {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let tick = ctx.signal(0_u64);
    let next = tick.clone();
    let interval = ctx.create_interval(TICK, move || next.update(|tick| *tick += 1));
    Self {
      tick,
      seen: Mutex::new(None),
      interval,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { run, again } = ctx.props::<Props>().clone();
    self.tick.get();
    self.interval.start();
    let now = merge_retry::waiting(run);
    let mut seen = self.seen.lock();
    if now != *seen {
      *seen = now;
      again.update(|count| *count += 1);
    }
    Column::new()
  }
}
