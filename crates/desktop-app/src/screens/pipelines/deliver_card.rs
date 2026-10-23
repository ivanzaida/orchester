//! The Deliver step on the canvas (`Card · 07 Deliver` `q0Woq0` in `qukzW`, the cards of `OqgbW`): always the last
//! card, numbered after the steps, its mode and method on the executor line, a badge for who merges (*Waits for your
//! approval* in `warning` for Approve to merge, *Merges when every step passed* for Auto-merge) and one naming the
//! integrator profile. The *Delivers* chip at the canvas's top right (`gURMA`) says how the pipeline delivers, or
//! that it has no Deliver step; a press on either opens the Deliver step's panel.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::{DeliverMode, DeliverStep};
use orchester_workspace::pipelines::deliver::integrating_step;

use super::{
  canvas::step_badge,
  deliver::executor_words,
  state::{Editing, Panel, State},
};
use crate::{
  theme::{DeliveryText, ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
  },
};

/// The Deliver card's height: a step card's with a second badge (`q0Woq0`).
pub(super) const DELIVER_HEIGHT: f32 = crate::theme::pipelines::CARD_HEIGHT + crate::theme::pipelines::GATE_HEIGHT;
/// The chip's height (`gURMA`).
const CHIP_HEIGHT: f32 = 36.0;

/// The Deliver card of `deliver`, the pipeline's last step, numbered `number`.
pub(super) fn card(ctx: &Ctx, state: &State, (deliver, number): (&DeliverStep, usize), width: f32) -> Element {
  let editing = state.get();
  let selected = editing.panel == Panel::Deliver;
  let (gate, gate_label) = match deliver.mode() {
    DeliverMode::ApproveToMerge => (Icon::Hand, ctx.t("delivery.step.badgeApprove")),
    DeliverMode::AutoMerge => (Icon::Zap, ctx.t("delivery.step.badgeAuto")),
  };
  let body = Column::new()
    .width(Dimension::Px(width))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Snug.role())
    .background(PaletteColor::SurfaceRaised)
    .rounded(RadiusSize::Lg)
    .border(Border::inside(
      BorderSize::Sm,
      if selected {
        ExtraColor::Selection.role()
      } else {
        PaletteColor::BorderStrong
      },
    ))
    .child(
      Row::new()
        .spacing(ExtraSpacing::Snug.role())
        .child(Text::new(&format!("{number:02}")).variant(ExtraTypography::MonoNote.role()))
        .child(
          Text::new(&ctx.t("delivery.step.title"))
            .id("pipeline-deliver-title")
            .variant(ExtraTypography::Strong.role()),
        ),
    )
    .child(
      Row::new()
        .spacing(ExtraSpacing::Pair.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::GitMerge, IconSize::Sm, PaletteColor::TextMuted))
        .child(
          Text::new(&executor_words(ctx, deliver))
            .id("pipeline-deliver-executor")
            .variant(ExtraTypography::Note.role())
            .color(PaletteColor::TextSecondary),
        ),
    )
    .child(step_badge(&gate_label, gate))
    .child(step_badge(
      &integrator_words(ctx, &editing, deliver),
      Icon::GitCompareArrows,
    ));
  let panels = editing.panels;
  pressable(move || panels.choose(Panel::Deliver))
    .id("pipeline-deliver")
    .describe("mode", mode_name(deliver.mode()))
    .child(body)
    .into()
}

/// *Conflicts: Builder*: the profile the Integrate step works with, or that conflicts go to the person.
fn integrator_words(ctx: &Ctx, editing: &Editing, deliver: &DeliverStep) -> std::sync::Arc<str> {
  let profile = editing
    .shown()
    .validate()
    .ok()
    .and_then(|pipeline| integrating_step(&pipeline, deliver, |_| true).and_then(|step| step.kind.executor()))
    .and_then(|profile| {
      editing
        .opened
        .assignables
        .profile(profile)
        .map(|choice| choice.name.clone())
    });
  match profile {
    Some(name) => ctx.t_args("delivery.step.badgeConflicts", [("profile", name.as_ref())]),
    None => ctx.t("delivery.step.badgeConflictsPerson"),
  }
}

/// The *Delivers* chip at the canvas's top right, which opens the Deliver step's panel.
pub(super) fn chip(ctx: &Ctx, state: &State) -> Element {
  let editing = state.get();
  let (glyph, color, words) = match editing.shown().deliver() {
    Some(deliver) => (
      Icon::GitMerge,
      PaletteColor::TextSecondary,
      match deliver.mode() {
        DeliverMode::ApproveToMerge => ctx.t("delivery.step.chipApprove"),
        DeliverMode::AutoMerge => ctx.t_args(
          "delivery.step.chipAuto",
          [(
            "method",
            crate::screens::merge_queue::words::method(ctx, deliver.method()),
          )],
        ),
      },
    ),
    None => (
      Icon::CircleDashed,
      PaletteColor::TextMuted,
      ctx.t("delivery.step.chipNone"),
    ),
  };
  let text_color = if editing.shown().deliver().is_some() {
    PaletteColor::TextPrimary
  } else {
    PaletteColor::TextSecondary
  };
  let panels = editing.panels;
  pressable(move || panels.choose(Panel::Deliver))
    .id("pipeline-deliver-chip")
    .height(Dimension::Px(CHIP_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Md))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(icon(glyph, IconSize::Md, color))
    .child(
      Text::new(&words)
        .id("pipeline-deliver-chip-label")
        .variant(DeliveryText::Chip.role())
        .color(text_color),
    )
    .into()
}

const fn mode_name(mode: DeliverMode) -> &'static str {
  match mode {
    DeliverMode::ApproveToMerge => "approve_to_merge",
    DeliverMode::AutoMerge => "auto_merge",
  }
}
