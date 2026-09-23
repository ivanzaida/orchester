//! Seconds left until a moment, as notice trailing text (`Q1TRov` "Next attempt in 8 s",
//! `R4Nsnb` "Closes in 5 s"). Its own component so the one-second tick re-renders only it.

use std::time::{Duration, Instant};

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::Text,
  core::Signal,
  node::Element,
};

const TICK: Duration = Duration::from_secs(1);

#[derive(Clone, PartialEq)]
pub struct CountdownProps {
  pub until: Instant,
  /// Message with a `{{seconds}}` placeholder while time is left.
  pub counting: &'static str,
  /// Message once the moment has passed.
  pub done: &'static str,
}

impl lurq::app::component::DevtoolsInspectable for CountdownProps {}

pub struct Countdown {
  tick: Signal<u64>,
}

impl Component for Countdown {
  type Props = CountdownProps;

  fn create(ctx: &mut Ctx) -> Self {
    let tick = ctx.signal(0_u64);
    let next = tick.clone();
    ctx
      .create_interval(TICK, move || next.update(|tick| *tick += 1))
      .start();
    Self { tick }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // Read so every tick re-renders the remaining time.
    self.tick.get();
    let props = ctx.props::<CountdownProps>().clone();
    let seconds = props
      .until
      .saturating_duration_since(Instant::now())
      .as_millis()
      .div_ceil(1000);
    let text = if seconds == 0 {
      ctx.t(props.done)
    } else {
      let seconds = seconds.to_string();
      ctx.t_args(props.counting, [("seconds", seconds.as_str())])
    };
    Text::new(&text)
      .variant(TypographyStyle::Caption)
      .color(PaletteColor::TextMuted)
      .nowrap()
  }
}
