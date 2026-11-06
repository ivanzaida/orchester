//! Words that count the seconds while a service runs, each in a small component of its own with its own one-second
//! tick, so only they draw again as the time goes: a running service's uptime (`18m 02s`, `pPX2f`) and what a
//! starting one waits for (`Waiting for HTTP 2xx on / · 12 s of 90 s`, `tWw1x`).

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Interval},
    theme::{PaletteColor, TypographyStyle},
  },
  components::Text,
  core::Signal,
  node::Element,
};
use orchester_core::service::Service;
use orchester_ipc::services::RunService;

use super::words;
use crate::theme::RunServicesText;

const TICK: Duration = Duration::from_secs(1);

/// What the words count.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Counted {
  /// How long the service has run.
  Uptime,
  /// What the starting service waits for, as its definition says.
  Waiting(Option<Arc<Service>>),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct SinceProps {
  pub id: Arc<str>,
  pub service: RunService,
  pub counted: Counted,
}

impl DevtoolsInspectable for SinceProps {}

pub(super) struct Since {
  tick: Signal<u64>,
  _interval: Interval,
}

impl Component for Since {
  type Props = SinceProps;

  fn create(ctx: &mut Ctx) -> Self {
    let tick = ctx.signal(0_u64);
    let next = tick.clone();
    let interval = ctx.create_interval(TICK, move || next.update(|tick| *tick += 1));
    interval.start();
    Self {
      tick,
      _interval: interval,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    self.tick.get();
    let SinceProps { id, service, counted } = ctx.props::<SinceProps>().clone();
    let ran = words::since(service.started_at);
    match counted {
      Counted::Uptime => Text::new(&words::uptime(ctx, ran))
        .id(id.to_string())
        .variant(RunServicesText::Uptime.role())
        .nowrap(),
      Counted::Waiting(definition) => Text::new(&words::waiting(ctx, &service, definition.as_deref(), ran))
        .id(id.to_string())
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    }
  }
}
