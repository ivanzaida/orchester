//! Context meter (ORC-305, `docs/design/context-meter.md`): how full an agent session's context window is, as a
//! 12 ring and the share in Geist Mono 11, 6 apart (`Context meter · states` `Kp4IZ`). In a composer's bar it is a
//! 24 high hit area, 8 at its sides, radius 6, with the `hover` fill under the pointer and the `pressed` fill while its
//! popover is open; on a step's divider it has no padding (`bxHFm`). Under 80 % the ring and share are `text_muted`,
//! from 80 % `warning`, at the whole window the ring is `danger` and *Full* takes the share's place; a CLI that reports
//! no window gets a dashed ring and no share. Resting on it shows its tooltip (*Context 86% · 172k of 200k*); a press
//! opens its popover ([`popover`]). *Compact* beside it is not part of this build (ORC-309): its place stays empty.

mod popover;
mod ring;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Placement, Popup, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  connection::context_use::{ContextUse, Fullness},
  tokens::compact_tokens,
};

use self::ring::{Ring, RingProps};
use super::{
  icon::{Icon, icon},
  pressable::pressable,
  tooltip::Tooltip,
};
use crate::theme::{ContextText, ControlHeight, ExtraColor, ExtraSpacing, IconSize};

/// Where the meter sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeterPlace {
  /// In a composer's bar: a padded hit area; the popover opens over it, from its end.
  Bar,
  /// On the divider of the step at work: no padding; the popover opens under it, from its start.
  Divider,
}

/// What a meter shows.
#[derive(Clone, Debug, PartialEq)]
pub struct ContextMeterProps {
  /// The element id of the meter; its parts' ids start with it.
  pub id: Arc<str>,
  /// The latest reading; `None` while the agent's CLI reported nothing.
  pub reading: Option<ContextUse>,
  /// The agent whose session it is, which a run's popover names before the model.
  pub agent: Option<Arc<str>>,
  pub model: Option<Arc<str>>,
  pub place: MeterPlace,
}

impl DevtoolsInspectable for ContextMeterProps {}

/// A reading as a meter's own read of it holds it: `None` while the agent's CLI reported nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeterReading(pub Option<ContextUse>);

impl DevtoolsInspectable for MeterReading {}

/// The meter for `props`, keyed by its id so its popover stays open while its reading changes.
pub fn context_meter(ctx: &mut Ctx, props: ContextMeterProps) -> Element {
  let key = format!("context-meter-{}", props.id);
  ctx.mount_keyed::<ContextMeter>(&key, props)
}

/// The name a meter's state is described by, for tools and tests.
pub const fn state_name(fullness: Fullness) -> &'static str {
  match fullness {
    Fullness::Plenty => "plenty",
    Fullness::NearFull => "near-full",
    Fullness::Full => "full",
    Fullness::Unknown => "unknown",
  }
}

/// The color of the ring's used part and of the share.
const fn share_color(fullness: Fullness) -> PaletteColor {
  match fullness {
    Fullness::NearFull => PaletteColor::Warning,
    Fullness::Full => PaletteColor::Danger,
    Fullness::Plenty | Fullness::Unknown => PaletteColor::TextMuted,
  }
}

struct ContextMeter {
  open: Signal<bool>,
  anchor: ElementRef,
}

impl Component for ContextMeter {
  type Props = ContextMeterProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ElementRef::new(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ContextMeterProps>().clone();
    let fullness = props.reading.map_or(Fullness::Unknown, |reading| reading.fullness());
    let open = self.open.get();
    let face = face(ctx, &props, fullness);
    let toggle = self.open.clone();
    let mut button = pressable(move || toggle.update(|open| *open = !*open))
      .id(props.id.to_string())
      .ref_element(self.anchor.clone())
      .describe("state", state_name(fullness))
      .describe("a11y_name", ctx.t("ui.contextMeter.name"))
      .flex_shrink(0.0)
      .child(face);
    if props.place == MeterPlace::Bar {
      let fill = if open {
        ExtraColor::Pressed.role()
      } else {
        ExtraColor::Hover.role()
      };
      button = button
        .height(Dimension::Px(ControlHeight::Sm.value()))
        .padding(Padding::horizontal(SpacingSize::Sm))
        .rounded(RadiusSize::Md)
        .hovered_style(Style::new().background(fill));
      if open {
        button = button.background(ExtraColor::Pressed.role());
      }
    }
    let tip = Tooltip::new(props.id.clone(), tooltip(ctx, props.reading, fullness))
      .silent(open)
      .wrap(ctx, button);
    let (placement, side) = match props.place {
      MeterPlace::Bar => (Placement::TopEnd, ExtraSpacing::Snug.value()),
      MeterPlace::Divider => (Placement::BottomStart, ExtraSpacing::Snug.value()),
    };
    let panel = popover::panel(ctx, &props, fullness);
    let popup = Popup::new(self.anchor.clone(), panel)
      .open(self.open.clone())
      .placement(placement)
      // Away from the meter: lurq moves an overlay above its anchor up, and one under it down, by a positive offset.
      .offset(0.0, side);
    Row::new()
      .flex_shrink(0.0)
      .align_items(Alignment::Center)
      .child(tip)
      .child(popup)
  }
}

/// The ring and the share, *Full*, or the dashed ring of an unknown share.
fn face(ctx: &mut Ctx, props: &ContextMeterProps, fullness: Fullness) -> Row {
  let row = Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center);
  let Some(share) = props.reading.and_then(|reading| reading.share()) else {
    return row.child(icon(Icon::CircleDashed, IconSize::Sm, PaletteColor::TextMuted).id(format!("{}-ring", props.id)));
  };
  let ring = ctx.mount_keyed::<Ring>(
    &format!("context-ring-{}", props.id),
    RingProps {
      id: format!("{}-ring", props.id).into(),
      share,
      color: share_color(fullness),
    },
  );
  let (words, variant) = if fullness == Fullness::Full {
    (ctx.t("ui.contextMeter.full"), ContextText::Full.role())
  } else {
    let share = share.to_string();
    (
      ctx.t_args("ui.contextMeter.share", [("share", share.as_str())]),
      ContextText::Share.role(),
    )
  };
  let text = Text::new(&words)
    .id(format!("{}-share", props.id))
    .variant(variant)
    .nowrap();
  let text = if fullness == Fullness::NearFull {
    text.color(PaletteColor::Warning)
  } else {
    text
  };
  row.child(ring).child(text)
}

/// What the tooltip says: the share and the tokens used of the window, or that the size is not reported.
fn tooltip(ctx: &Ctx, reading: Option<ContextUse>, fullness: Fullness) -> Arc<str> {
  let Some(reading) = reading else {
    return ctx.t("ui.contextMeter.tooltipUnknown");
  };
  let used = compact_tokens(reading.used());
  let (Some(window), Some(share)) = (reading.window(), reading.share()) else {
    return ctx.t_args("ui.contextMeter.tooltipUsedOnly", [("used", used.as_str())]);
  };
  let (window, share) = (compact_tokens(window), share.to_string());
  let args = [
    ("used", used.as_str()),
    ("window", window.as_str()),
    ("share", share.as_str()),
  ];
  let key = match (fullness, reading.estimated()) {
    (Fullness::Full, _) => "ui.contextMeter.tooltipFull",
    (_, true) => "ui.contextMeter.tooltipEstimate",
    (_, false) => "ui.contextMeter.tooltip",
  };
  ctx.t_args(key, args)
}
