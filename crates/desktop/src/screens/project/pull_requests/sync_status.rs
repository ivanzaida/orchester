//! The topbar sync status (`k5p3Tg`): "Synced 2 min ago", "Rate limited · resets in 12 min"
//! and the other states of [`SyncLine`]. Its own component with its own interval, so the
//! relative time stays current without re-rendering the list. A rate limit's exact reset time
//! shows in a tooltip while the pointer is over the text (`Wn3fn`).

use std::time::Duration;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, Placement},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Popup, Row, Text},
  core::{ElementRef, Signal},
  node::{Element, HitTestBehavior, border::Border},
};
use orchester_core::time::Timestamp;

use super::view::SyncLine;
use crate::theme::{ExtraShadow, ExtraSpacing};

/// Relative times are shown in whole minutes; a half-minute tick keeps them at most that stale.
const TICK: Duration = Duration::from_secs(30);
/// Space between the status text and its tooltip.
const TOOLTIP_GAP: f32 = 6.0;

#[derive(Clone, PartialEq, Eq)]
pub struct SyncStatusProps {
  pub line: SyncLine,
}

impl lurq::app::component::DevtoolsInspectable for SyncStatusProps {}

pub struct SyncStatus {
  tick: Signal<u64>,
  hovered: Signal<bool>,
  anchor: ElementRef,
}

impl Component for SyncStatus {
  type Props = SyncStatusProps;

  fn create(ctx: &mut Ctx) -> Self {
    let tick = ctx.signal(0_u64);
    let next = tick.clone();
    ctx
      .create_interval(TICK, move || next.update(|tick| *tick += 1))
      .start();
    Self {
      tick,
      hovered: ctx.signal(false),
      anchor: ElementRef::new(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // Read so every tick re-renders the relative time.
    self.tick.get();
    let line = ctx.props::<SyncStatusProps>().line.clone();
    let text = line.text(ctx, Timestamp::now());
    let (enter, leave) = (self.hovered.clone(), self.hovered.clone());
    let status = Row::new()
      .id("pull-requests-sync-status")
      .ref_element(self.anchor.clone())
      .on_mouse_enter(move || enter.set(true))
      .on_mouse_leave(move || leave.set(false))
      .child(Text::new(&text).variant(TypographyStyle::Caption).nowrap());
    let tooltip = line.reset_hint(ctx).filter(|_| self.hovered.get()).map(|hint| {
      let panel = Column::new()
        .id("pull-requests-reset-time")
        .padding_vertical(ExtraSpacing::Tight.role())
        .padding_horizontal(SpacingSize::Sm)
        .background(PaletteColor::SurfaceRaised)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .rounded(RadiusSize::Md)
        .box_shadow(ExtraShadow::Popover.role())
        .child(
          Text::new(&hint)
            .variant(TypographyStyle::Caption)
            .color(PaletteColor::TextPrimary)
            .nowrap(),
        );
      Popup::new(self.anchor.clone(), panel)
        .open_when(true)
        .placement(Placement::BottomEnd)
        .offset(0.0, TOOLTIP_GAP)
        .hit_test(HitTestBehavior::None)
        .dismiss_on_outside_click(false)
        .dismiss_on_escape(false)
    });
    Row::new().child(status).with_children(tooltip)
  }
}
