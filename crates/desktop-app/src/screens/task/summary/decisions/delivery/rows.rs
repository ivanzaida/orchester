//! The parts the task page's delivery views share: a line of the delivery's verification (`Delivery check row`
//! `QyqsJ`: a 14 glyph, what was checked over what came of it, and a time or revision at the end), a file that
//! conflicted (`Conflict file row` `Nanwh`), and the run's stages reduced to *01–06 Every step passed*, *Integrate*
//! and *07 Deliver* (`Stages` in `ulEuh`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::merge_queue::{QueueEvent, QueueState};
use orchester_workspace::{
  display::{clock, short_revision},
  merge_queue::{DeliveryCheck, RunDelivery},
  tasks::detail::RunView,
};

use super::super::stages::stage_chip;
use crate::{
  theme::{DeliveryText, ExtraSpacing, ExtraTypography, IconSize},
  ui::icon::{Icon, icon},
};

/// A conflicting file's row (`Nanwh`).
const CONFLICT_ROW_HEIGHT: f32 = 34.0;

/// How a line of the verification looks: its glyph in its color, its title's color and its detail's.
#[derive(Clone)]
pub(super) struct Look {
  pub glyph: Icon,
  pub color: PaletteColor,
  pub title: PaletteColor,
  pub detail: PaletteColor,
}

impl Look {
  pub(super) const DONE: Self = Self {
    glyph: Icon::CircleCheck,
    color: PaletteColor::Success,
    title: PaletteColor::TextPrimary,
    detail: PaletteColor::TextSecondary,
  };
  pub(super) const WORKING: Self = Self {
    glyph: Icon::LoaderCircle,
    color: PaletteColor::TextSecondary,
    title: PaletteColor::TextPrimary,
    detail: PaletteColor::TextSecondary,
  };
  pub(super) const PENDING: Self = Self {
    glyph: Icon::CircleDashed,
    color: PaletteColor::TextMuted,
    title: PaletteColor::TextMuted,
    detail: PaletteColor::TextSecondary,
  };
  /// What failed says why in `danger` too (`ZOr8X`, `NILKO` in `e7geY3`).
  pub(super) const FAILED: Self = Self {
    glyph: Icon::CircleX,
    color: PaletteColor::Danger,
    title: PaletteColor::TextPrimary,
    detail: PaletteColor::Danger,
  };
  pub(super) const CONFLICTED: Self = Self {
    glyph: Icon::GitCompareArrows,
    color: PaletteColor::TextSecondary,
    title: PaletteColor::TextPrimary,
    detail: PaletteColor::TextSecondary,
  };
}

/// A line of the verification: `title` over `detail`, `end` at its end.
pub(super) fn check_row(id: &str, look: Look, (title, detail): (&str, &str), end: Option<&str>) -> Element {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Fine.role(), ExtraSpacing::Snug.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(icon(look.glyph, IconSize::Base, look.color))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Hair.role())
        .child(Text::new(title).variant(TypographyStyle::Label).color(look.title))
        .child(
          Text::new(detail)
            .id(format!("{id}-detail"))
            .variant(ExtraTypography::Note.role())
            .color(look.detail),
        ),
    )
    .with_children(end.map(|end| Text::new(end).variant(DeliveryText::Stage.role()).nowrap()))
    .into()
}

/// The lines `delivery` adds to the run's verification, `base` and `integrator` named in them.
pub(super) fn delivery_checks(ctx: &Ctx, delivery: &RunDelivery, (base, integrator): (&str, &str)) -> Vec<Element> {
  orchester_workspace::merge_queue::checks(delivery)
    .into_iter()
    .enumerate()
    .map(|(index, check)| {
      let id = format!("task-delivery-check-{index}");
      delivery_check(ctx, &id, &check, (base, integrator))
    })
    .collect()
}

fn delivery_check(ctx: &Ctx, id: &str, check: &DeliveryCheck, (base, integrator): (&str, &str)) -> Element {
  let sync_title = ctx.t_args("delivery.task.check.sync", [("base", base)]);
  let check_title = ctx.t("delivery.task.check.check");
  match check {
    DeliveryCheck::Synced {
      base: revision,
      conflicts,
      resyncs,
      at,
    } => {
      let args = [
        ("base", base.to_owned()),
        ("revision", short_revision(revision.as_str()).to_owned()),
        ("count", conflicts.to_string()),
        ("resyncs", resyncs.to_string()),
      ];
      let key = match (*conflicts, *resyncs) {
        (0, 0) => "delivery.task.check.synced",
        (0, _) => "delivery.task.check.resynced",
        _ => "delivery.task.check.syncedConflicts",
      };
      let look = if *conflicts > 0 { Look::CONFLICTED } else { Look::DONE };
      check_row(id, look, (&sync_title, &ctx.t_args(key, args)), Some(&clock(*at)))
    }
    DeliveryCheck::Integrating { files, since } => {
      let title = ctx.t_args("delivery.task.check.integrate", [("integrator", integrator)]);
      let detail = ctx.t_args("delivery.task.check.integrating", [("count", files.to_string())]);
      check_row(id, Look::WORKING, (&title, &detail), Some(&clock(*since)))
    }
    DeliveryCheck::Integrated { files } => {
      let title = ctx.t_args("delivery.task.check.integrate", [("integrator", integrator)]);
      let detail = ctx.t_args("delivery.task.check.integrated", [("count", files.to_string())]);
      check_row(id, Look::DONE, (&title, &detail), None)
    }
    DeliveryCheck::Checking { base: revision, since } => {
      let detail = ctx.t_args("delivery.task.check.checking", [("time", clock(*since))]);
      let end = format!("{base} {}", short_revision(revision.as_str()));
      check_row(id, Look::WORKING, (&check_title, &detail), Some(&end))
    }
    DeliveryCheck::CheckWaits => check_row(
      id,
      Look::PENDING,
      (&check_title, &ctx.t("delivery.task.check.waits")),
      None,
    ),
    DeliveryCheck::Checked {
      base: revision, tool, ..
    } => {
      let detail = ctx.t_args("delivery.task.check.passed", [("tool", tool.as_str())]);
      let end = format!("{base} {}", short_revision(revision.as_str()));
      check_row(id, Look::DONE, (&check_title, &detail), Some(&end))
    }
    DeliveryCheck::CheckFailed {
      base: revision, tool, ..
    } => {
      let detail = ctx.t_args("delivery.task.check.failed", [("tool", tool.as_str())]);
      let end = format!("{base} {}", short_revision(revision.as_str()));
      check_row(id, Look::FAILED, (&check_title, &detail), Some(&end))
    }
    DeliveryCheck::Approved { at } => {
      let title = ctx.t("delivery.task.check.approval");
      let detail = ctx.t_args("delivery.task.check.approved", [("time", clock(*at))]);
      check_row(id, Look::DONE, (&title, &detail), None)
    }
  }
}

/// A file that conflicted: `look`'s glyph, its path and how it stands.
pub(super) fn conflict_row(id: &str, look: Look, path: &str, state: &str) -> Element {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(CONFLICT_ROW_HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(look.glyph, IconSize::Base, look.color))
    .child(Text::new(path).variant(TypographyStyle::Mono).flex(1.0).nowrap())
    .child(
      Text::new(state)
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .into()
}

/// How the run's delivery stands, for its stages.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Stage {
  /// Being delivered.
  Working,
  /// Waiting for the person.
  Waiting,
  /// Merged.
  Done,
}

/// *01–06 Every step passed*, *Integrate* when the run's conflicts went to it, and *07 Deliver*, as `stage` says.
pub(super) fn stages(ctx: &Ctx, run: &RunView, delivery: Option<&RunDelivery>, stage: Stage) -> Element {
  let steps = run.steps.len();
  let range = if steps > 1 {
    format!("01\u{2013}{steps:02}")
  } else {
    "01".to_owned()
  };
  let passed = ctx.t("delivery.task.stage.passed");
  let done = || (Icon::CircleCheck, PaletteColor::Success);
  let mut chips = vec![stage_chip(
    "task-stage-passed",
    (Some(&range), &passed),
    (done(), PaletteColor::TextSecondary, PaletteColor::Border),
  )];
  if let Some(integrate) = delivery.and_then(integrate_look) {
    let (glyph, name, stroke) = integrate;
    chips.push(stage_chip(
      "task-stage-integrate",
      (None, &ctx.t("delivery.task.stage.integrate")),
      (glyph, name, stroke),
    ));
  }
  let deliver = match stage {
    Stage::Working => (
      (Icon::LoaderCircle, PaletteColor::TextPrimary),
      PaletteColor::TextPrimary,
      PaletteColor::Border,
    ),
    Stage::Waiting => (
      (Icon::Hand, PaletteColor::Warning),
      PaletteColor::Warning,
      PaletteColor::Warning,
    ),
    Stage::Done => (done(), PaletteColor::TextSecondary, PaletteColor::Border),
  };
  let number = format!("{:02}", steps + 1);
  chips.push(stage_chip(
    "task-stage-deliver",
    (Some(&number), &ctx.t("delivery.task.stage.deliver")),
    deliver,
  ));
  Row::new()
    .id("task-stages")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .wrap()
    .with_children(chips)
    .into()
}

type ChipLook = ((Icon, PaletteColor), PaletteColor, PaletteColor);

/// The *Integrate* chip's look, when the run's conflicts went to its Integrate step.
fn integrate_look(delivery: &RunDelivery) -> Option<ChipLook> {
  let integrated = delivery
    .events
    .iter()
    .any(|recorded| matches!(recorded.event, QueueEvent::IntegrateStarted { .. }));
  match &delivery.entry.state {
    QueueState::Integrating { .. } => Some((
      (Icon::LoaderCircle, PaletteColor::TextPrimary),
      PaletteColor::TextPrimary,
      PaletteColor::Border,
    )),
    QueueState::Left(orchester_core::merge_queue::Left::Conflicts { .. }) => Some((
      (Icon::CircleX, PaletteColor::Danger),
      PaletteColor::Danger,
      PaletteColor::Danger,
    )),
    _ if integrated => Some((
      (Icon::CircleCheck, PaletteColor::Success),
      PaletteColor::TextSecondary,
      PaletteColor::Border,
    )),
    _ => None,
  }
}

/// "since 16:58".
pub(super) fn since(ctx: &Ctx, at: orchester_core::time::Timestamp) -> Arc<str> {
  ctx.t_args("delivery.queue.since", [("time", clock(at))])
}
