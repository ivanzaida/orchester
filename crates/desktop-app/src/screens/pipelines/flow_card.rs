//! A card of the flow (`03 Implement` `v6LUcW` in `IsU5u`): its number, its name, who runs it (profile ·
//! connection / model, or *Tool · name*) and what comes of it; under a step that sends work back, where it goes
//! (`Changes requested → 03 Implement`, `Failure route` `Mdx77`). The chosen card takes the `hover` fill in a
//! `border_focus` outline; while the publish review or a comparison shows, a changed card says so (`Changed` `X4LDh`).
//! The Deliver card is numbered after the steps; without one, a chip says where runs end (`No Deliver step`
//! `a9kcuk`).
use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::{DeliverMode, DeliverStep, RepositoryAccess, Step, StepKind};
use orchester_workspace::pipelines::{deliver::integrating_step, describe::executor_line, narrowing::check_step};

use super::state::{Editing, View};
use crate::{
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, PipelinesText},
  ui::{
    icon::{Icon, icon},
    message::message_text,
    pressable::pressable,
    row_group::tag,
  },
};

/// The card of `step`, the `index`th of the steps.
pub fn step(ctx: &Ctx, editing: &Editing, (step, index): (&Step, usize)) -> Element {
  let chosen = editing.view.step() == Some(&step.key);
  let invalid = check_step(step, &editing.opened.assignables).is_err()
    || matches!(
      step.kind,
      StepKind::Agent { profile: None, .. } | StepKind::Tool { tool: None }
    );
  let changed = reference(editing).is_some_and(|before| before.steps().iter().find(|s| s.key == step.key) != Some(step));
  let executor = Text::new(&message_text(ctx, &executor_line(step, &editing.opened.assignables)))
    .id(format!("pipeline-step-executor-{}", step.key))
    .variant(TypographyStyle::Description)
    .color(if invalid { PaletteColor::Danger } else { PaletteColor::TextSecondary })
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .min_width(Dimension::Px(0.0));
  let (outcome, warning) = outcome(ctx, step, invalid);
  let mut text = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(title(ctx, step, (invalid, changed)))
    .child(executor)
    .child(line(&format!("pipeline-step-outcome-{}", step.key), &outcome, warning));
  if let Some(route) = &step.on_failure {
    let shown = editing.shown();
    let target = shown
      .steps()
      .iter()
      .enumerate()
      .find(|(_, s)| s.key == route.return_to)
      .map_or_else(String::new, |(index, s)| format!("{:02} {}", index + 1, s.name));
    text = text.child(
      Row::new()
        .id(format!("pipeline-failure-route-{}", step.key))
        .spacing(ExtraSpacing::Pair.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::CornerDownLeft, IconSize::Sm, PaletteColor::TextMuted))
        .child(
          Text::new(&ctx.t_args("pipelines.failureRoute", [("step", target)]))
            .variant(TypographyStyle::Description)
            .color(PaletteColor::TextMuted),
        ),
    );
  }
  let (views, key) = (editing.views.clone(), step.key.clone());
  card(
    pressable(move || views.choose(View::Step(key.clone()))).id(format!("pipeline-step-{}", step.key)),
    (index + 1, chosen),
    text,
  )
}

/// What the card's step is compared with: the draft while a version is compared, the published version in the
/// publish review.
fn reference(editing: &Editing) -> Option<&orchester_core::resource::Pipeline> {
  if editing.comparing {
    Some(&editing.opened.body)
  } else if editing.view == View::Publish {
    editing.opened.latest.as_ref().map(|v| &v.body)
  } else {
    None
  }
}

fn title(ctx: &Ctx, step: &Step, (invalid, changed): (bool, bool)) -> Row {
  Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(step.name.as_str())
        .id(format!("pipeline-step-title-{}", step.key))
        .describe("a11y_name", step.name.as_str().to_owned())
        .variant(ExtraTypography::Strong.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .min_width(Dimension::Px(0.0)),
    )
    .with_children(changed.then(|| tag(&format!("pipeline-step-changed-{}", step.key), &ctx.t("pipelines.changed"), true)))
    .with_children(invalid.then(|| icon(Icon::CircleAlert, IconSize::Sm, PaletteColor::Danger)))
}

/// What comes of the step, and whether it waits for the person (`warning`).
fn outcome(ctx: &Ctx, step: &Step, invalid: bool) -> (Arc<str>, bool) {
  if step.approval_gate {
    return (ctx.t("pipelines.gateBadge"), true);
  }
  let key = match step.kind {
    StepKind::Tool { .. } if invalid => "pipelines.toolUnavailableBadge",
    StepKind::Tool { .. } => "pipelines.toolBadge",
    StepKind::Agent {
      repository: RepositoryAccess::ReadOnly,
      ..
    } => "pipelines.readBadge",
    StepKind::Agent { .. } => "pipelines.writeBadge",
  };
  (ctx.t(key), false)
}

fn line(id: &str, words: &str, warning: bool) -> Text {
  Text::new(words)
    .id(id.to_owned())
    .variant(TypographyStyle::Description)
    .color(if warning { PaletteColor::Warning } else { PaletteColor::TextMuted })
}

/// A card's frame: 9 over and under, 12 at its sides, its number 10 before its words, in a `border` outline,
/// radius 6; chosen, the `hover` fill in a `border_focus` outline.
fn card(button: Button, (number, chosen): (usize, bool), text: Column) -> Element {
  let button = button
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Start)
    .rounded(RadiusSize::Md)
    .border_inside(
      BorderSize::Sm,
      if chosen { PaletteColor::BorderFocus } else { PaletteColor::Border },
    )
    .describe("chosen", if chosen { "true" } else { "false" });
  let button = if chosen {
    button.background(ExtraColor::Hover.role())
  } else {
    button
  };
  button
    .child(Text::new(&format!("{number:02}")).variant(PipelinesText::CardNumber.role()))
    .child(text)
    .into()
}

/// The Deliver card, numbered after the steps, or the chip that says the pipeline has none.
pub fn deliver(ctx: &Ctx, editing: &Editing) -> Element {
  let shown = editing.shown();
  let views = editing.views.clone();
  let Some(deliver) = shown.deliver() else {
    return pressable(move || views.choose(View::Deliver))
      .id("pipeline-deliver-chip")
      .padding(Padding::symmetric(ExtraSpacing::Loose.role(), ExtraSpacing::Pair.role()))
      .spacing(ExtraSpacing::Snug.role())
      .rounded(ExtraRadius::Full.role())
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .child(icon(Icon::GitMerge, IconSize::Sm, PaletteColor::TextMuted))
      .child(
        Text::new(&ctx.t("pipelines.noDeliverChip"))
          .id("pipeline-deliver-chip-label")
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextMuted),
      )
      .into();
  };
  let changed = reference(editing).is_some_and(|before| before.deliver() != Some(deliver));
  let (outcome, warning) = deliver_outcome(ctx, editing, deliver);
  let text = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(
      Row::new()
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(
          Text::new(&ctx.t("delivery.step.title"))
            .id("pipeline-deliver-title")
            .variant(ExtraTypography::Strong.role()),
        )
        .with_children(changed.then(|| tag("pipeline-deliver-changed", &ctx.t("pipelines.changed"), true))),
    )
    .child(
      Text::new(&super::deliver::executor_words(ctx, deliver))
        .id("pipeline-deliver-executor")
        .variant(TypographyStyle::Description),
    )
    .child(line("pipeline-deliver-outcome", &outcome, warning));
  card(
    pressable(move || views.choose(View::Deliver))
      .id("pipeline-deliver")
      .describe("mode", mode_name(deliver.mode())),
    (shown.steps().len() + 1, editing.view == View::Deliver),
    text,
  )
}

/// Who merges and who resolves conflicts (`Waits for your approval · Conflicts: Builder`).
fn deliver_outcome(ctx: &Ctx, editing: &Editing, deliver: &DeliverStep) -> (Arc<str>, bool) {
  let (mode, warning) = match deliver.mode() {
    DeliverMode::ApproveToMerge => (ctx.t("delivery.step.badgeApprove"), true),
    DeliverMode::AutoMerge => (ctx.t("delivery.step.badgeAuto"), false),
  };
  let profile = editing
    .shown()
    .validate()
    .ok()
    .and_then(|pipeline| integrating_step(&pipeline, deliver, |_| true).and_then(|step| step.kind.executor()))
    .and_then(|profile| editing.opened.assignables.profile(profile).map(|choice| choice.name.clone()));
  let conflicts = match profile {
    Some(name) => ctx.t_args("delivery.step.badgeConflicts", [("profile", name.as_ref())]),
    None => ctx.t("delivery.step.badgeConflictsPerson"),
  };
  (
    ctx.t_args(
      "pipelines.deliverOutcome",
      [("mode", mode.to_string()), ("conflicts", conflicts.to_string())],
    ),
    warning,
  )
}

const fn mode_name(mode: DeliverMode) -> &'static str {
  match mode {
    DeliverMode::ApproveToMerge => "approve_to_merge",
    DeliverMode::AutoMerge => "auto_merge",
  }
}
