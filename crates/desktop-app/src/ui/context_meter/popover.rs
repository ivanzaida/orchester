//! The meter's popover (`Popover` `v6vdj`, `fJ1Bn`, `yIbCQ`, `OZGiw`; on a run `yCk2o`): `surface_raised` in a
//! `border_strong` stroke, radius 8, 12 inside and 10 between its parts, lifted by the menu's shadow. Its head says
//! *Context*, the state's badge (*Near full*, *Full*) and, at its end, the model, or the agent and its model on a run;
//! then a 4 high bar of the share, the tokens used of the window and the share. An estimated window marks both with
//! ≈ and says why; an unknown share says so. What fills the window and *Compact* are not part of this build: no agent
//! reports the first, and Compact comes with ORC-309.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, ShadowStyle, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::context_use::{ContextUse, Fullness},
  tokens::compact_tokens,
};

use super::{ContextMeterProps, share_color};
use crate::{
  theme::{ContextText, ExtraRadius, ExtraSpacing},
  ui::{
    chip::{BadgeTone, badge},
    meter::{MeterTone, meter},
  },
};

/// The popover's width (`Z2dtIW`'s `n34Doh`), and on a run, whose head names the agent too (`yCk2o`).
const WIDTH: f32 = 300.0;
const WIDE: f32 = 360.0;

/// The popover's panel for `props`, whose reading is `fullness` full.
pub(super) fn panel(ctx: &Ctx, props: &ContextMeterProps, fullness: Fullness) -> Element {
  let id = format!("{}-popover", props.id);
  let width = if props.agent.is_some() { WIDE } else { WIDTH };
  let parts: Vec<Element> = match props.reading {
    Some(reading) if reading.window().is_some() => known(ctx, &id, reading, fullness),
    reading => unknown(ctx, &id, reading),
  };
  Column::new()
    .id(id.clone())
    .describe("state", super::state_name(fullness))
    .width(Dimension::Px(width))
    .padding(SpacingSize::Md)
    .spacing(ExtraSpacing::Loose.role())
    .rounded(ExtraRadius::Tile.role())
    .background(PaletteColor::SurfaceRaised)
    .border_inside(BorderSize::Sm, PaletteColor::BorderStrong)
    .box_shadow(ShadowStyle::Md)
    .child(head(ctx, &id, props, fullness))
    .with_children(parts)
    .into()
}

/// *Context*, the state's badge, and the model at the end.
fn head(ctx: &Ctx, id: &str, props: &ContextMeterProps, fullness: Fullness) -> Element {
  let state = match fullness {
    Fullness::NearFull => Some(badge(
      &format!("{id}-badge"),
      BadgeTone::Warning,
      &ctx.t("ui.contextMeter.nearFull"),
    )),
    Fullness::Full => Some(badge(
      &format!("{id}-badge"),
      BadgeTone::Danger,
      &ctx.t("ui.contextMeter.full"),
    )),
    Fullness::Plenty | Fullness::Unknown => None,
  };
  let model: Option<Arc<str>> = match (&props.agent, &props.model) {
    (Some(agent), Some(model)) => {
      Some(ctx.t_args("ui.contextMeter.agentModel", [("agent", &**agent), ("model", &**model)]))
    }
    (Some(only), None) | (None, Some(only)) => Some(only.clone()),
    (None, None) => None,
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("ui.contextMeter.name"))
        .id(format!("{id}-title"))
        .variant(ContextText::Title.role())
        .nowrap(),
    )
    .with_children(state)
    .child(Spacer::new().flex(1.0))
    .with_children(model.map(|model| {
      Text::new(&model)
        .id(format!("{id}-model"))
        .variant(ContextText::Model.role())
        .nowrap()
    }))
    .into()
}

/// The bar, the tokens used of the window and the share; an estimate marked and explained.
fn known(ctx: &Ctx, id: &str, reading: ContextUse, fullness: Fullness) -> Vec<Element> {
  let (Some(window), Some(share)) = (reading.window(), reading.share()) else {
    return Vec::new();
  };
  let tone = match fullness {
    Fullness::NearFull => MeterTone::Reached,
    Fullness::Full => MeterTone::Full,
    Fullness::Plenty | Fullness::Unknown => MeterTone::Quiet,
  };
  let (used, window_words) = (compact_tokens(reading.used()), compact_tokens(window));
  let figures = [("used", used.as_str()), ("window", window_words.as_str())];
  let share_words = share.to_string();
  let (tokens, share) = if reading.estimated() {
    (
      ctx.t_args("ui.contextMeter.usedEstimate", figures),
      ctx.t_args("ui.contextMeter.shareEstimate", [("share", share_words.as_str())]),
    )
  } else {
    (
      ctx.t_args("ui.contextMeter.used", figures),
      ctx.t_args("ui.contextMeter.share", [("share", share_words.as_str())]),
    )
  };
  let line = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&tokens)
        .id(format!("{id}-tokens"))
        .variant(ContextText::Tokens.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(
      Text::new(&share)
        .id(format!("{id}-share"))
        .variant(ContextText::Share.role())
        .color(share_color(fullness))
        .nowrap(),
    );
  let mut parts = vec![meter(&format!("{id}-bar"), (reading.used(), window), tone), line.into()];
  if reading.estimated() {
    parts.push(note(id, &ctx.t("ui.contextMeter.estimateNote")));
  }
  parts
}

/// No share to show: the tokens used where the CLI counted them, and why there is no share.
fn unknown(ctx: &Ctx, id: &str, reading: Option<ContextUse>) -> Vec<Element> {
  let used = reading.map(|reading| {
    let used = compact_tokens(reading.used());
    Text::new(&ctx.t_args("ui.contextMeter.usedOnly", [("used", used.as_str())]))
      .id(format!("{id}-tokens"))
      .variant(ContextText::Tokens.role())
      .width(Dimension::Pct(100.0))
      .into()
  });
  let why = if reading.is_some() {
    "ui.contextMeter.windowNotReported"
  } else {
    "ui.contextMeter.notReported"
  };
  used.into_iter().chain([note(id, &ctx.t(why))]).collect()
}

fn note(id: &str, words: &str) -> Element {
  Text::new(words)
    .id(format!("{id}-note"))
    .variant(ContextText::Note.role())
    .width(Dimension::Pct(100.0))
    .into()
}
