//! Times in the shell that change while they are shown: how long an operation has run, how long an
//! agent has worked on its step, and the countdown to the runner's next reconnect attempt. Each is a
//! component of its own with its own interval, so a tick renders one text and nothing else; a time that
//! does not grow (an agent that waits for a person) does not tick.

use std::time::{Duration, Instant};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Interval},
    theme::TypographyStyle,
  },
  components::Text,
  core::Signal,
  node::Element,
};
use orchester_core::{run::ActiveClock, time::Timestamp};
use orchester_workspace::display;

use crate::{theme::ExtraTypography, ui::message::message_text};

const TICK: Duration = Duration::from_secs(1);

/// A time that changes while it is shown.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Clock {
  /// How long an operation has run (*4 s*, *3 min*).
  Since(Instant),
  /// An agent's active time on its step (*11m*), which grows only while the agent works.
  Active(ActiveClock),
  /// The runner's reconnect attempt `attempt` of `limit`, and when the next one starts (*attempt 2 of 5
  /// · next in 8 s*); `None` or past while an attempt runs.
  Retry {
    attempt: u32,
    limit: u32,
    next: Option<Instant>,
  },
}

impl Clock {
  const fn ticks(&self) -> bool {
    match self {
      Self::Since(_) | Self::Retry { .. } => true,
      Self::Active(clock) => clock.counting(),
    }
  }

  fn text(&self, ctx: &Ctx) -> String {
    match *self {
      Self::Since(since) => message_text(ctx, &display::duration(since.elapsed())).to_string(),
      Self::Active(clock) => compact(ctx, clock.at(Timestamp::now())),
      Self::Retry { attempt, limit, next } => retry(ctx, attempt, limit, next),
    }
  }
}

/// *11m*, *40s*, *2h*: a step's active time as the sidebar's Running list shows it (`QDtA3`).
fn compact(ctx: &Ctx, time: Duration) -> String {
  let seconds = time.as_secs();
  let (key, count) = match seconds {
    0..60 => ("shell.time.seconds", seconds),
    60..3600 => ("shell.time.minutes", seconds / 60),
    _ => ("shell.time.hours", seconds / 3600),
  };
  ctx.t_args(key, [("count", count.to_string())]).to_string()
}

fn retry(ctx: &Ctx, attempt: u32, limit: u32, next: Option<Instant>) -> String {
  let seconds = next.map_or(0, |next| {
    next
      .saturating_duration_since(Instant::now())
      .as_millis()
      .div_ceil(1000)
  });
  let (attempt, limit) = (attempt.to_string(), limit.to_string());
  if seconds == 0 {
    return ctx
      .t_args(
        "shell.status.runner.attempting",
        [("attempt", attempt), ("limit", limit)],
      )
      .to_string();
  }
  ctx
    .t_args(
      "shell.status.runner.nextAttempt",
      [("attempt", attempt), ("limit", limit), ("seconds", seconds.to_string())],
    )
    .to_string()
}

impl DevtoolsInspectable for Clock {}

/// `clock` in 11 `text_muted`, with the element id `id`.
pub fn clock(ctx: &mut Ctx, id: &str, clock: Clock) -> Element {
  ctx.mount_keyed::<Ticking>(
    &format!("clock-{id}"),
    Ticked {
      id: id.to_owned(),
      clock,
    },
  )
}

#[derive(Clone, Debug, PartialEq)]
struct Ticked {
  id: String,
  clock: Clock,
}

impl DevtoolsInspectable for Ticked {}

struct Ticking {
  tick: Signal<u64>,
  interval: Interval,
}

impl Component for Ticking {
  type Props = Ticked;

  fn create(ctx: &mut Ctx) -> Self {
    let tick = ctx.signal(0_u64);
    let next = tick.clone();
    let interval = ctx.create_interval(TICK, move || next.update(|tick| *tick += 1));
    Self { tick, interval }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Ticked { id, clock } = ctx.props::<Ticked>().clone();
    // Read so every tick renders the time again.
    self.tick.get();
    if clock.ticks() {
      self.interval.start();
    } else {
      self.interval.stop();
    }
    // An agent's time in the Running list is Geist Mono (`QDtA3`); the status bar's times are its labels'.
    let style = match clock {
      Clock::Active(_) => ExtraTypography::MonoNote.role(),
      Clock::Since(_) | Clock::Retry { .. } => TypographyStyle::Caption,
    };
    Text::new(&clock.text(ctx)).id(id).variant(style).nowrap()
  }
}
