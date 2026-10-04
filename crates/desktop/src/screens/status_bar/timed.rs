//! Values that change every second: the elapsed time of a running operation and the
//! countdown to the next reconnect attempt. Each is its own component with its own interval,
//! mounted only while it is shown, so a tick re-renders one text and nothing else.

use std::time::{Duration, Instant};

use lurq::{
  app::{component::Component, ctx::Ctx, theme::PaletteColor},
  core::Signal,
  node::Element,
};

use super::item::value;
use crate::{display, ui::message_text};

const TICK: Duration = Duration::from_secs(1);

/// A signal that changes every second while its component is mounted.
pub(super) fn ticking(ctx: &mut Ctx) -> Signal<u64> {
  let tick = ctx.signal(0_u64);
  let next = tick.clone();
  ctx
    .create_interval(TICK, move || next.update(|tick| *tick += 1))
    .start();
  tick
}

/// How long an operation has run (`w3dC6` "4 s", `Odh3s`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ElapsedProps {
  pub since: Instant,
}

impl lurq::app::component::DevtoolsInspectable for ElapsedProps {}

pub struct Elapsed {
  tick: Signal<u64>,
}

impl Component for Elapsed {
  type Props = ElapsedProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self { tick: ticking(ctx) }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // Read so every tick re-renders the time.
    self.tick.get();
    let since = ctx.props::<ElapsedProps>().since;
    let text = message_text(ctx, &display::duration(since.elapsed()));
    value(&text, PaletteColor::TextMuted)
  }
}

/// Reconnect attempt and the time to the next one (`y5NHKO` "attempt 2 of 5 · next in 8 s").
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RetryProps {
  pub attempt: u32,
  pub limit: u32,
  /// When the next attempt starts; `None` or past while an attempt runs.
  pub next: Option<Instant>,
}

impl lurq::app::component::DevtoolsInspectable for RetryProps {}

pub struct Retry {
  tick: Signal<u64>,
}

impl Component for Retry {
  type Props = RetryProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self { tick: ticking(ctx) }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    self.tick.get();
    let props = *ctx.props::<RetryProps>();
    let seconds = props.next.map_or(0, |next| {
      next
        .saturating_duration_since(Instant::now())
        .as_millis()
        .div_ceil(1000)
    });
    let (attempt, limit) = (props.attempt.to_string(), props.limit.to_string());
    let text = if seconds == 0 {
      ctx.t_args(
        "statusBar.runner.attempting",
        [("attempt", attempt.as_str()), ("limit", limit.as_str())],
      )
    } else {
      let seconds = seconds.to_string();
      ctx.t_args(
        "statusBar.runner.nextAttempt",
        [
          ("attempt", attempt.as_str()),
          ("limit", limit.as_str()),
          ("seconds", seconds.as_str()),
        ],
      )
    };
    value(&text, PaletteColor::TextMuted)
  }
}
