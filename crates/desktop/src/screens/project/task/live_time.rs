//! Active agent time that goes on counting while an agent of the run works: in "This run"
//! (`F5Lsi`) and in the progress rows of the steps at work. Each is a component of its own whose
//! interval runs, once a second, only while what it shows counts, so a page with no agent at work,
//! or in a window tab that is offstage (lurq runs no timers there), schedules no redraws. Each run
//! event reads the run again, and the value starts over from what the runner recorded.

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, Interval},
  },
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{run::ActiveClock, time::Timestamp};

use super::progress::steps_column;
use crate::{
  clock::Clock,
  display,
  tasks::progress::ProgressRow,
  ui::{message_text, property::property},
};

const TICK: Duration = Duration::from_secs(1);

/// The time live values are shown at.
pub(super) fn clock(ctx: &mut Ctx) -> Clock {
  ctx
    .use_context::<Clock>()
    .expect("the root provides the Clock before mounting screens")
}

/// A tick once a second, while a value counts.
struct Ticker {
  tick: Signal<u64>,
  interval: Interval,
}

impl Ticker {
  fn new(ctx: &mut Ctx) -> Self {
    let tick = ctx.signal(0_u64);
    let next = tick.clone();
    let interval = ctx.create_interval(TICK, move || next.update(|tick| *tick += 1));
    Self { tick, interval }
  }

  /// The time to show values at; the ticks go on while `counting` and stop otherwise.
  fn now(&self, ctx: &mut Ctx, counting: bool) -> Timestamp {
    if counting {
      self.interval.start();
      // Read so every tick renders the value again.
      self.tick.get();
    } else {
      self.interval.stop();
    }
    clock(ctx).now()
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ActiveTimeProps {
  pub(super) active: ActiveClock,
}

impl lurq::app::component::DevtoolsInspectable for ActiveTimeProps {}

/// The run's *Active agent time* row of "This run".
pub(super) struct ActiveTime {
  ticker: Ticker,
}

impl Component for ActiveTime {
  type Props = ActiveTimeProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      ticker: Ticker::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let active = ctx.props::<ActiveTimeProps>().active;
    let now = self.ticker.now(ctx, active.counting());
    let time = message_text(ctx, &display::duration(active.at(now)));
    Column::new()
      .id("task-active-time")
      .width(Dimension::Pct(100.0))
      .child(property(&ctx.t("runs.summary.time"), &time))
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LiveStepsProps {
  pub(super) rows: Vec<ProgressRow>,
  /// What each row's agent waits on, replacing its detail.
  pub(super) waiting_on: Vec<Option<Arc<str>>>,
}

impl lurq::app::component::DevtoolsInspectable for LiveStepsProps {}

/// The progress rows of the run's steps (`qB7mj`).
pub(super) struct LiveSteps {
  ticker: Ticker,
}

impl Component for LiveSteps {
  type Props = LiveStepsProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      ticker: Ticker::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<LiveStepsProps>().clone();
    let counting = props
      .rows
      .iter()
      .any(|row| row.usage.is_some_and(|usage| usage.active.counting()));
    let now = self.ticker.now(ctx, counting);
    steps_column(ctx, &props.rows, &props.waiting_on, now)
  }
}
