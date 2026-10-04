//! The step rail collapsed to a 56 strip (`kfntH` in `I35q7D`): its head opens the full rail
//! (panel-right-open), then *All* and a badge per step — its state icon over its number, 40 × 44,
//! `$hover` while picked, the live step outlined. A badge picks its step as the rail's row does.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, ScrollVertical, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, border::Border, dimension::Dimension},
};

use super::{
  StepFilter,
  rail::{HEAD_HEIGHT, RailToggle, glyph},
};
use crate::{
  tasks::timeline::rail::{RailState, RailStep},
  theme::{ExtraColor, ExtraSpacing, NOT_STARTED_OPACITY},
  ui::{
    icon::{Icon, IconSize, icon},
    pressable::{pressable, surface},
  },
};

/// The strip's width and a badge's size (`kfntH`, `ZNWkf`).
pub(super) const STRIP_WIDTH: f32 = 56.0;
const BADGE_WIDTH: f32 = 40.0;
const BADGE_HEIGHT: f32 = 44.0;

/// The strip of `steps`; `window` decides whether its head opens the rail in place or over the chat.
pub(super) fn strip(
  ctx: &Ctx,
  steps: &[RailStep],
  (filter, toggle): (&Signal<StepFilter>, &RailToggle),
  window: f32,
) -> Element {
  let picked = filter.get().0;
  let expand = pressable(surface().id("conversation-rail-expand"), toggle.expand(window))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEAD_HEIGHT))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(icon(Icon::PanelRightOpen, IconSize::Event, PaletteColor::TextSecondary));
  let all = badge(
    "conversation-strip-all".into(),
    (Icon::List, ctx.t("runs.conversation.rail.allShort").to_string()),
    (picked.is_none(), false),
    Some(pick(filter, StepFilter(None))),
  );
  let badges = steps.iter().map(|step| {
    let live = matches!(step.state, RailState::Working | RailState::Waits(_));
    let press = step.started().then(|| pick(filter, StepFilter(Some(step.key.clone()))));
    badge(
      format!("conversation-strip-step-{}", step.number),
      (glyph(&step.state), format!("{:02}", step.number)),
      (picked.as_ref() == Some(&step.key), live),
      press,
    )
  });
  let list = Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(ExtraSpacing::Inline.role())
    .padding_horizontal(SpacingSize::Sm)
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(all)
    .with_children(badges);
  Column::new()
    .id("conversation-strip")
    .width(Dimension::Px(STRIP_WIDTH))
    .height(Dimension::Pct(100.0))
    .flex_shrink(0.0)
    .background(PaletteColor::SurfacePanel)
    .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(expand)
    .child(ScrollVertical::new(list).width(Dimension::Pct(100.0)).flex(1.0))
    .into()
}

fn pick(filter: &Signal<StepFilter>, key: StepFilter) -> Box<dyn Fn() + Send + Sync> {
  let filter = filter.clone();
  Box::new(move || filter.set(key.clone()))
}

/// A badge: its icon over its label; pressable unless its step has not started.
fn badge(
  id: String,
  (glyph, label): (Icon, String),
  (picked, live): (bool, bool),
  press: Option<Box<dyn Fn() + Send + Sync>>,
) -> Element {
  let tone = if picked || live {
    PaletteColor::TextPrimary
  } else {
    PaletteColor::TextSecondary
  };
  let number = if picked || live {
    PaletteColor::TextPrimary
  } else {
    PaletteColor::TextMuted
  };
  let content = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Small, tone))
    .child(Text::new(&label).variant(TypographyStyle::Mono).color(number).nowrap());
  let Some(press) = press else {
    return Column::new()
      .id(id)
      .size(Dimension::Px(BADGE_WIDTH), Dimension::Px(BADGE_HEIGHT))
      .justify(Justify::Center)
      .opacity(NOT_STARTED_OPACITY)
      .child(content)
      .into();
  };
  let badge = pressable(surface().id(id), press)
    .size(Dimension::Px(BADGE_WIDTH), Dimension::Px(BADGE_HEIGHT))
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Sm)
    .hovered_style(Style::new().background(PaletteColor::AccentMuted))
    .describe("picked", picked.to_string())
    .child(content);
  let badge = if live {
    badge.border(Border::inside(BorderSize::Sm, PaletteColor::Border))
  } else {
    badge
  };
  if picked {
    badge.background(PaletteColor::AccentMuted).into()
  } else {
    badge.into()
  }
}
