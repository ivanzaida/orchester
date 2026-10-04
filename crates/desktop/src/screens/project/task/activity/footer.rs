//! The working line, the chat's last row while an agent works (`VCcdr` in `kpnmP`), indented to a
//! message's text: who is working and how long ago the last entry arrived, a real time that its own
//! interval keeps current, never progress of its own.

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::time::Timestamp;

use crate::{
  display,
  theme::ExtraSpacing,
  ui::{
    icon::{Icon, IconSize, icon},
    message_text,
  },
};

/// The line's height and how often the elapsed time is refreshed.
const FOOTER_HEIGHT: f32 = 40.0;
const TICK: Duration = Duration::from_secs(1);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::screens::project::task) struct LiveFooterProps {
  pub(in crate::screens::project::task) agent: Arc<str>,
  /// When the newest entry arrived; `None` before the first.
  pub(in crate::screens::project::task) last: Option<Timestamp>,
}

impl lurq::app::component::DevtoolsInspectable for LiveFooterProps {}

pub(in crate::screens::project::task) struct LiveFooter {
  tick: Signal<u64>,
}

impl Component for LiveFooter {
  type Props = LiveFooterProps;

  fn create(ctx: &mut Ctx) -> Self {
    let tick = ctx.signal(0_u64);
    let next = tick.clone();
    ctx
      .create_interval(TICK, move || next.update(|tick| *tick += 1))
      .start();
    Self { tick }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // Read so every tick re-renders the elapsed time.
    self.tick.get();
    let props = ctx.props::<LiveFooterProps>().clone();
    let text = match props.last {
      Some(last) => {
        // A clock set back since the entry arrived reads as just now.
        let millis = u64::try_from(Timestamp::now().unix_millis() - last.unix_millis()).unwrap_or_default();
        let elapsed = message_text(ctx, &display::duration(Duration::from_millis(millis)));
        ctx.t_args(
          "runs.activity.working",
          [("agent", &*props.agent), ("elapsed", &*elapsed)],
        )
      }
      None => ctx.t_args("runs.activity.workingStarted", [("agent", &*props.agent)]),
    };
    Row::new()
      .id("activity-live")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(FOOTER_HEIGHT))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .padding_left(ExtraSpacing::TimelineIndent.role())
      .child(icon(Icon::Loader, IconSize::Small, PaletteColor::TextSecondary))
      .child(Text::new(&text).variant(TypographyStyle::Caption))
  }
}
