//! Returns and restarts in the transcript (02.B7 `zyusO`): a row of its own before the header of
//! the step a new attempt starts from, like a problem row (`N4SlQq`) without the time gutter. An
//! automatic return or restart says why it happened, with a muted glyph and "automatic" first in
//! its meta; a person's decision has a primary glyph, "by you", and a return carries their note.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  resource::StepKey,
  run::{Attempt, AttemptReason},
  time::Timestamp,
};

use crate::{
  display,
  tasks::detail::RunView,
  theme::{ExtraSpacing, ExtraTypography},
  ui::icon::{Icon, IconSize, icon},
};

/// The glyph tile, as a transcript entry's (`DQmo9`).
const GLYPH_SIZE: f32 = 24.0;

/// Why an attempt started, as its mark reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum MarkKind {
  /// A failure's route returned the work to `to`; `retry` is which retry of how many the route
  /// allows, for an automatic one.
  Returned {
    to: Arc<str>,
    agent: Arc<str>,
    failed: Arc<str>,
    failed_attempt: u32,
    retry: (usize, u32),
    /// The other steps of the attempt, which run again.
    after: Vec<Arc<str>>,
  },
  /// `step` broke and starts again as a fresh session.
  Restarted { step: Arc<str> },
  /// A person returned the work to `to` with `note`.
  ReturnedWithNote { to: Arc<str>, note: Arc<str> },
}

/// A return or restart before the header of the step its attempt starts from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Mark {
  pub(super) attempt: u32,
  pub(super) at: Option<Timestamp>,
  pub(super) by_person: bool,
  pub(super) kind: MarkKind,
}

/// The mark of the attempt at `index` of `run`'s attempts, when it is a return or a restart.
pub(super) fn mark_of(run: &RunView, index: usize) -> Option<Mark> {
  let attempts = run.run.attempts();
  let attempt = attempts.get(index)?;
  let start = run.attempt_starts.iter().find(|start| start.attempt == attempt.id());
  let name = |step: &StepKey| -> Arc<str> {
    run
      .steps
      .iter()
      .find(|view| view.key == *step)
      .map_or_else(|| step.as_str().into(), |view| view.name.clone())
  };
  let kind = match attempt.reason() {
    AttemptReason::Retry { failed_step, return_to } => MarkKind::Returned {
      to: name(return_to),
      agent: run
        .steps
        .iter()
        .find(|view| view.key == *return_to)
        .and_then(|view| view.agent.clone())
        .unwrap_or_else(|| name(return_to)),
      failed: name(failed_step),
      failed_attempt: index
        .checked_sub(1)
        .map_or(0, |earlier| attempts[earlier].number().get()),
      retry: retry_of(run, &attempts[..=index], failed_step),
      after: others(attempt, return_to).map(name).collect(),
    },
    AttemptReason::Restart { step } => MarkKind::Restarted { step: name(step) },
    AttemptReason::Returned { return_to, note, .. } => MarkKind::ReturnedWithNote {
      to: name(return_to),
      note: note.as_str().into(),
    },
    AttemptReason::Initial | AttemptReason::FollowUp { .. } | AttemptReason::Recovery { .. } => return None,
  };
  Some(Mark {
    attempt: attempt.number().get(),
    at: start.map(|start| start.at),
    by_person: start.is_some_and(|start| start.by_person),
    kind,
  })
}

/// Which retry of `failed_step`'s route `attempts` ends with, and how many the route allows.
fn retry_of(run: &RunView, attempts: &[Attempt], failed_step: &StepKey) -> (usize, u32) {
  let taken = attempts
    .iter()
    .filter(|attempt| matches!(attempt.reason(), AttemptReason::Retry { failed_step: step, .. } if step == failed_step))
    .count();
  let allowed = run
    .run
    .snapshot()
    .body()
    .step(failed_step)
    .and_then(|step| step.on_failure.as_ref())
    .map_or(0, |route| route.max_retries.get());
  (taken, allowed)
}

fn others<'a>(attempt: &'a Attempt, start: &'a StepKey) -> impl Iterator<Item = &'a StepKey> {
  attempt
    .executions()
    .iter()
    .map(|execution| execution.step())
    .filter(move |step| *step != start)
}

/// The mark's row: glyph, title and meta, and what follows from it.
pub(super) fn mark_row(ctx: &Ctx, mark: &Mark) -> Element {
  let (glyph, title, detail) = text(ctx, mark);
  let note = matches!(mark.kind, MarkKind::ReturnedWithNote { .. });
  let tone = if mark.by_person || note {
    PaletteColor::TextPrimary
  } else {
    PaletteColor::TextSecondary
  };
  let heading = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(GLYPH_SIZE))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&title)
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextPrimary)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .min_width(Dimension::Px(0.0))
        .flex(1.0),
    )
    .child(
      Text::new(&meta(ctx, mark))
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextMuted)
        .nowrap()
        .flex_shrink(0.0),
    );
  Row::new()
    .id(format!("activity-mark-{}", mark.attempt))
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Sm)
    .spacing(ExtraSpacing::Stack.role())
    .child(
      Row::new()
        .size(Dimension::Px(GLYPH_SIZE), Dimension::Px(GLYPH_SIZE))
        .flex_shrink(0.0)
        .background(PaletteColor::SurfaceRaised)
        .rounded(RadiusSize::Sm)
        .align_items(Alignment::Center)
        .justify(Justify::Center)
        .child(icon(glyph, IconSize::Entry, tone)),
    )
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(heading)
        .child(
          Text::new(&detail)
            .id(format!("activity-mark-detail-{}", mark.attempt))
            .width(Dimension::Pct(100.0))
            .variant(ExtraTypography::Explanation.role())
            .color(if note {
              PaletteColor::TextPrimary
            } else {
              PaletteColor::TextSecondary
            })
            .selectable(true),
        ),
    )
    .into()
}

/// "automatic · retry 2 of 2 · 14:39", "automatic · attempt 2 · 12:16" or "by you · 14:52".
fn meta(ctx: &Ctx, mark: &Mark) -> Arc<str> {
  let time = mark.at.map(display::clock).unwrap_or_default();
  match (&mark.kind, mark.by_person) {
    (_, true) | (MarkKind::ReturnedWithNote { .. }, _) => {
      ctx.t_args("runs.activity.mark.byYou", [("time", time.as_str())])
    }
    (
      MarkKind::Returned {
        retry: (taken, allowed),
        ..
      },
      false,
    ) => ctx.t_args(
      "runs.activity.mark.automaticRetry",
      [
        ("retry", taken.to_string().as_str()),
        ("max", allowed.to_string().as_str()),
        ("time", time.as_str()),
      ],
    ),
    (MarkKind::Restarted { .. }, false) => ctx.t_args(
      "runs.activity.mark.automaticAttempt",
      [("attempt", mark.attempt.to_string().as_str()), ("time", time.as_str())],
    ),
  }
}

/// The mark's glyph, title and explanation.
fn text(ctx: &Ctx, mark: &Mark) -> (Icon, Arc<str>, Arc<str>) {
  match &mark.kind {
    MarkKind::Returned {
      to,
      agent,
      failed,
      failed_attempt,
      after,
      ..
    } => {
      let attempt = failed_attempt.to_string();
      let title = ctx.t_args(
        "runs.activity.mark.returnedTitle",
        [("step", &**to), ("failed", &**failed), ("attempt", attempt.as_str())],
      );
      let detail = if after.is_empty() {
        ctx.t_args(
          "runs.activity.mark.returnedDetailAlone",
          [("agent", &**agent), ("failed", &**failed)],
        )
      } else {
        let steps = after.join(&*ctx.t("runs.activity.mark.and"));
        ctx.t_args(
          "runs.activity.mark.returnedDetail",
          [("agent", &**agent), ("failed", &**failed), ("steps", steps.as_str())],
        )
      };
      (Icon::Undo2, title, detail)
    }
    MarkKind::Restarted { step } => (
      Icon::RotateCcw,
      ctx.t_args("runs.activity.mark.restartedTitle", [("step", &**step)]),
      ctx.t("runs.activity.mark.restartedDetail"),
    ),
    MarkKind::ReturnedWithNote { to, note } => (
      Icon::Undo2,
      ctx.t_args("runs.activity.mark.returnedWithNoteTitle", [("step", &**to)]),
      ctx.t_args("runs.activity.mark.note", [("note", &**note)]),
    ),
  }
}
