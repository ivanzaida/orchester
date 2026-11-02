//! Context meter (ORC-305, `docs/design/context-meter.md`): how full an agent session's context window is, as a
//! 12 ring and the share in Geist Mono 11, 6 apart (`Context meter · states` `Kp4IZ`). In a composer's bar it is a
//! 24 high hit area, 8 at its sides, radius 6, with the `hover` fill under the pointer and the `pressed` fill while its
//! popover is open; on a step's divider it has no padding (`bxHFm`). Under 80 % the ring and share are `text_muted`,
//! from 80 % `warning`, at the whole window the ring is `danger` and *Full* takes the share's place; a CLI that reports
//! no window gets a dashed ring and no share. Resting on it shows its tooltip (*Context 86% · 172k of 200k*); a press
//! opens its popover ([`popover`]), above the composer's box in a bar ([`Over`]). Where the session can be compacted
//! (ORC-309, [`Compact`]), *Compact* sits beside
//! the bar's meter from 80 % (ghost, secondary when full) and closes its popover; while the session is compacted the
//! meter is a `loader-circle` and *Compacting…* in `text_secondary` (`gY3Ku`).

mod popover;
mod ring;

use std::{fmt, sync::Arc};

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
  button::{ActionButton, ButtonKind},
  icon::{Icon, icon},
  pressable::pressable,
  tooltip::Tooltip,
};
use crate::theme::{ContextText, ControlHeight, ExtraColor, ExtraSpacing, IconSize};

/// Where the meter sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeterPlace {
  /// In a composer's bar: a padded hit area; the popover opens above the composer's box ([`Over`]), else over the
  /// meter, from its end.
  Bar,
  /// On the divider of the step at work: no padding; the popover opens under it, from its start.
  Divider,
}

/// The composer's box a bar meter's popover opens above (`Z2dtIW`'s `n34Doh`): its bottom 6 over the box's top, its
/// right edge on the box's, clear of the box's words and controls.
#[derive(Clone)]
pub struct Over(pub ElementRef);

/// Compared as equal: the box is its composer's for as long as the meter is shown.
impl PartialEq for Over {
  fn eq(&self, _other: &Self) -> bool {
    true
  }
}

impl fmt::Debug for Over {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str("composer box")
  }
}

/// What *Compact* can do for the meter's session (ORC-309).
#[derive(Clone)]
pub enum Compact {
  /// The session cannot be compacted from here: no *Compact* is offered.
  NotOffered,
  /// Offered, but a turn is under way: it waits, disabled, and says so.
  Waits,
  /// A press compacts the session.
  Ready(Arc<dyn Fn() + Send + Sync>),
  /// The session is being compacted.
  Running,
}

impl Compact {
  const fn name(&self) -> &'static str {
    match self {
      Self::NotOffered => "not-offered",
      Self::Waits => "waits",
      Self::Ready(_) => "ready",
      Self::Running => "running",
    }
  }
}

/// Compared by what it offers; the press is the screen's.
impl PartialEq for Compact {
  fn eq(&self, other: &Self) -> bool {
    self.name() == other.name()
  }
}

impl fmt::Debug for Compact {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(self.name())
  }
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
  /// In a bar, the composer's box its popover opens above; `None` opens it over the meter.
  pub over: Option<Over>,
  pub compact: Compact,
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
    if props.compact == Compact::Running {
      return compacting(ctx, &props);
    }
    let fullness = props.reading.map_or(Fullness::Unknown, |reading| reading.fullness());
    let open = self.open.get();
    let face = face(ctx, &props, fullness);
    let toggle = self.open.clone();
    let mut button = pressable(move || toggle.update(|open| *open = !*open))
      .id(props.id.to_string())
      .ref_element(self.anchor.clone())
      .describe("state", state_name(fullness))
      .describe("compact", props.compact.name())
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
    let compact = closing(&props.compact, &self.open);
    let panel = popover::panel(ctx, &props, fullness, &compact);
    let popup = self.popup(&props, panel);
    // From near full, *Compact* sits beside the bar's meter (`GENCC`): ghost, secondary once the window is full.
    let beside = match (props.place, fullness) {
      (MeterPlace::Bar, Fullness::NearFull) => compact_button(ctx, &props.id, (ButtonKind::Ghost, true), &compact),
      (MeterPlace::Bar, Fullness::Full) => compact_button(ctx, &props.id, (ButtonKind::Secondary, true), &compact),
      _ => None,
    };
    Element::from(
      Row::new()
        .flex_shrink(0.0)
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(tip)
        .child(popup)
        .with_children(beside),
    )
  }
}

impl ContextMeter {
  /// The popover holding `panel`: above the composer's box in a bar that names it, else over the meter from its end
  /// in a bar and under it from its start on a divider; 6 away from what it opens at either way.
  fn popup(&self, props: &ContextMeterProps, panel: Element) -> Popup {
    let (anchor, placement, trigger) = match (props.place, &props.over) {
      // The box is no trigger: a press on the meter in it opens and closes the popover, one on the field leaves it.
      (MeterPlace::Bar, Some(Over(composer))) => (composer.clone(), Placement::TopEnd, false),
      (MeterPlace::Bar, None) => (self.anchor.clone(), Placement::TopEnd, true),
      (MeterPlace::Divider, _) => (self.anchor.clone(), Placement::BottomStart, true),
    };
    Popup::new(anchor, panel)
      .open(self.open.clone())
      .switch_on_anchor_press(trigger)
      .placement(placement)
      // Away from its anchor: lurq moves an overlay above its anchor up, and one under it down, by a positive offset.
      .offset(0.0, ExtraSpacing::Snug.value())
  }
}

/// `compact`, its press closing the popover `open` first.
fn closing(compact: &Compact, open: &Signal<bool>) -> Compact {
  match compact {
    Compact::Ready(press) => {
      let (press, open) = (press.clone(), open.clone());
      Compact::Ready(Arc::new(move || {
        open.set(false);
        press();
      }))
    }
    other => other.clone(),
  }
}

/// *Compact* of `kind`, named after `id`, the bar's helper size `in_bar` (`GENCC`: a 12 icon, 11.5 words), else a
/// button's own: a press compacts the session; while a turn is under way it waits, disabled, and its tooltip says so.
/// `None` where nothing is offered.
pub(super) fn compact_button(
  ctx: &mut Ctx,
  id: &str,
  (kind, in_bar): (ButtonKind, bool),
  compact: &Compact,
) -> Option<Element> {
  let button =
    ActionButton::new(format!("{id}-compact"), kind, ctx.t("ui.contextMeter.compact")).icon(Icon::FoldVertical);
  let button = if in_bar { button.compact() } else { button };
  match compact {
    Compact::Ready(press) => {
      let press = press.clone();
      Some(button.on_click(move || press()).into())
    }
    Compact::Waits => Some(
      Tooltip::new(format!("{id}-compact-waits"), ctx.t("ui.contextMeter.compactWaits")).wrap(ctx, button.disabled()),
    ),
    Compact::NotOffered | Compact::Running => None,
  }
}

/// The meter while its session is compacted (`gY3Ku`): a `loader-circle` and *Compacting…* in `text_secondary`.
fn compacting(ctx: &Ctx, props: &ContextMeterProps) -> Element {
  let row = Row::new()
    .id(props.id.to_string())
    .describe("state", "compacting")
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::LoaderCircle, IconSize::Sm, PaletteColor::TextSecondary))
    .child(
      Text::new(&ctx.t("ui.contextMeter.compacting"))
        .id(format!("{}-share", props.id))
        .variant(ContextText::Compacting.role())
        .nowrap(),
    );
  match props.place {
    MeterPlace::Bar => row
      .height(Dimension::Px(ControlHeight::Sm.value()))
      .padding(Padding::horizontal(SpacingSize::Sm))
      .into(),
    MeterPlace::Divider => row.into(),
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
