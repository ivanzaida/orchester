//! Values that go on counting while an agent of the run works: its active agent time in *This run* and
//! in the progress rows of the steps at work. Each is a component of its own whose interval runs, once
//! a second, only while what it shows counts, so a page with no agent at work, or a window tab that is
//! offstage (lurq runs no timers there), schedules no redraws. Each change of the run reads it again,
//! and the value starts over from what the runner recorded.

use std::time::Duration;

use lurq::{
  app::ctx::{Ctx, Interval},
  core::Signal,
};
use orchester_core::time::Timestamp;

use crate::services::Services;

const TICK: Duration = Duration::from_secs(1);

/// A tick once a second, while a value counts.
pub struct Ticker {
  tick: Signal<u64>,
  interval: Interval,
}

impl Ticker {
  pub fn new(ctx: &mut Ctx) -> Self {
    let tick = ctx.signal(0_u64);
    let next = tick.clone();
    let interval = ctx.create_interval(TICK, move || next.update(|tick| *tick += 1));
    Self { tick, interval }
  }

  /// The time to show values at; the ticks go on while `counting` and stop otherwise.
  pub fn now(&self, ctx: &mut Ctx, counting: bool) -> Timestamp {
    if counting {
      self.interval.start();
      // Read so every tick renders the value again.
      self.tick.get();
    } else {
      self.interval.stop();
    }
    Services::of(ctx).clock.now()
  }
}
