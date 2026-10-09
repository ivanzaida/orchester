//! A step's result card (`Result card` `T0uKNU`): on `surface_panel` in a `border` stroke, radius 10, 12 by
//! 14 inside, its parts 8 apart: a 14 `circle-check` in `success` (`circle-x` in `danger` when it failed),
//! *Review passed* in 12.5/600 and the step and agent at the end in 11.5 `text_muted`; what was reported in
//! 12.5/1.5 `text_secondary`; how long it took and its tokens, and *Changes ↗* and *Summary ↗* in 11.5/500
//! `accent_text`, 12 apart.

use std::time::Duration;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{run::StepOutcome, tokens::compact_tokens};
use orchester_workspace::{display, tasks::timeline::ResultCard};

use super::{Timeline, divider::step_label};
use crate::{
  routes::TaskView,
  theme::{ConversationText, ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    markdown::MarkdownView,
    message::message_text,
    pressable::pressable,
  },
};

pub(super) fn card(ctx: &mut Ctx, timeline: &Timeline, card: &ResultCard) -> Element {
  let id = format!("conversation-result-{}", card.execution);
  let (glyph, color) = if card.passed {
    (Icon::CircleCheck, PaletteColor::Success)
  } else {
    (Icon::CircleX, PaletteColor::Danger)
  };
  let title_key = match (card.passed, card.verdict) {
    (true, true) => "conversation.result.passed",
    (true, false) => "conversation.result.done",
    (false, _) => "conversation.result.failed",
  };
  let step = step_label(ctx, card.number, &card.step);
  let step = match &card.agent {
    Some(agent) => ctx.t_args("conversation.result.step", [("step", &*step), ("agent", &**agent)]),
    None => step,
  };
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Md, color))
    .child(
      Text::new(&ctx.t_args(title_key, [("step", &*card.step)]))
        .id(format!("{id}-title"))
        .variant(ConversationText::ResultTitle.role())
        .nowrap(),
    )
    .child(Spacer::new().flex(1.0))
    .child(Text::new(&step).variant(ExtraTypography::Note.role()).nowrap());
  let report = card.reported.as_ref().map(|reported| {
    MarkdownView::new(format!("{id}-report"), reported.clone(), super::message::links())
      .style(ExtraTypography::Lead.role(), PaletteColor::TextSecondary)
      .render(ctx)
  });
  let meta = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(
      Text::new(&meta(ctx, card))
        .id(format!("{id}-meta"))
        .variant(ExtraTypography::Note.role()),
    )
    .child(Spacer::new().flex(1.0))
    .child(link(
      ctx,
      timeline,
      (&format!("{id}-changes"), "conversation.result.changes"),
      TaskView::Changes(None),
    ))
    .child(link(
      ctx,
      timeline,
      (&format!("{id}-summary"), "conversation.result.summary"),
      TaskView::Summary,
    ));
  Column::new()
    .id(id)
    .describe("passed", card.passed.to_string())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(head)
    .with_children(report)
    .child(meta)
    .into()
}

/// *Reviewer reported passed · 6 min · 41k tokens*: what the agent's result said, how long the step took
/// and its tokens; a tool step's time, and that it used none.
fn meta(ctx: &Ctx, card: &ResultCard) -> std::sync::Arc<str> {
  // A clock set back while the step ran reads as no time, never as a negative one.
  let took = card.took_millis.map(|millis| {
    message_text(
      ctx,
      &display::duration(Duration::from_millis(u64::try_from(millis).unwrap_or_default())),
    )
  });
  let meta = match (took, card.tool_step) {
    (Some(took), false) => ctx.t_args(
      "conversation.result.meta",
      [("took", &*took), ("tokens", compact_tokens(card.tokens).as_str())],
    ),
    (Some(took), true) => ctx.t_args("conversation.result.toolMeta", [("took", &*took)]),
    (None, _) => "".into(),
  };
  // What the agent said needs the agent's name: a step whose profile is gone says nothing of it.
  let (Some(agent), false) = (&card.agent, card.tool_step) else {
    return meta;
  };
  let reported = match card.outcome {
    Some(StepOutcome::Done) => "conversation.result.reportedPassed",
    Some(StepOutcome::Failed) => "conversation.result.reportedFailed",
    Some(StepOutcome::Question) | None => return meta,
  };
  let reported = ctx.t_args(reported, [("agent", &**agent)]);
  if meta.is_empty() {
    return reported;
  }
  ctx.t_args(
    "conversation.result.withReport",
    [("reported", &*reported), ("meta", &*meta)],
  )
}

/// A link to `view` of the task.
fn link(ctx: &mut Ctx, timeline: &Timeline, (id, key): (&str, &str), view: TaskView) -> Element {
  let words = ctx.t(key);
  pressable(timeline.open(ctx, view))
    .id(id.to_owned())
    .child(
      Text::new(&words)
        .variant(ExtraTypography::NoteStrong.role())
        .color(ExtraColor::AccentText.role())
        .nowrap(),
    )
    .into()
}
