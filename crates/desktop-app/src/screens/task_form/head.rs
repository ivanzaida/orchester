//! The composer's head row (`Crumb row` `grDKk`): `← Tasks / New task`, or the task's key in Geist Mono for Edit task,
//! in 12 `text_muted`, and at its end the save status, a 12 icon and its words in 12: what the form holds and what it
//! is doing (Not saved yet, Unsaved changes in `warning`, Draft saved locally, Queued · run not started). Only what
//! happened is said: a task is "queued" once it is, a run "starting" while the runner is asked. Under the head a
//! one-line note on `surface_raised` says what the person should know first (`Note` `CvMGI`): that the task already
//! ran, that it is queued, or where kept changes came from.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Row, Spacer, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{display, tasks::run::StartOutcome};

use super::{
  Nav,
  state::{Phase, Subject, View},
};
use crate::{
  theme::{ExtraSpacing, IconSize},
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    icon::{Icon, icon},
    notice::{Notice, NoticeTone},
  },
};

pub fn head(ctx: &mut Ctx, view: &View, nav: &Nav) -> Element {
  Row::new()
    .id("task-form-head")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::ArrowLeft, IconSize::Base, PaletteColor::TextMuted))
    .child(crumbs(ctx, view, nav))
    .child(Spacer::new().flex(1.0))
    .child(save_status(ctx, view))
    .into()
}

fn crumbs(ctx: &mut Ctx, view: &View, nav: &Nav) -> Row {
  let tasks = Crumb::to(ctx.t("task_form.crumbs.tasks"), nav.to_tasks());
  let here = match view.subject.task() {
    None => Crumb::here(ctx.t("task_form.crumbs.newTask")),
    Some(task) if view.subject.awaits_first_run() => Crumb::here(task.key().to_string()).mono(),
    Some(task) => Crumb::to(task.key().to_string(), nav.to_task(task.key())).mono(),
  };
  breadcrumb(ctx, "task-form-crumbs", CrumbSeparator::Slash, vec![tasks, here])
}

/// What the person should know before the title, if anything: the task already ran, it is queued, or the changes were
/// kept from an earlier visit. A queued task opened again without a run says so in a notice (`t0fDw`).
pub fn notes(ctx: &Ctx, view: &View) -> Vec<Element> {
  if let Some(notice) = no_run_notice(ctx, view) {
    return vec![notice];
  }
  let queued = match &view.phase {
    Phase::Starting(queued) => Some(ctx.t_args("task_form.queued.savedAs", [("key", queued.key.to_string())])),
    Phase::NotStarted { queued, .. } => Some(ctx.t_args(
      match queued.outcome {
        StartOutcome::ConfirmedNotStarted => "task_form.queued.description",
        StartOutcome::Unconfirmed => "task_form.queued.unconfirmedDescription",
      },
      [("key", queued.key.to_string())],
    )),
    Phase::Editing | Phase::Saving | Phase::Creating => None,
  };
  if let Some(words) = queued {
    return vec![note("task-form-queued-note", Icon::CirclePause, &words)];
  }
  let mut notes = Vec::new();
  if let Subject::Ran(task) = &view.subject {
    let words = ctx.t_args("task_form.notes.ran", [("key", task.key().to_string())]);
    notes.push(note("task-form-ran-note", Icon::History, &words));
  }
  if let Some(at) = view.kept_at.filter(|_| view.changed) {
    let key = match view.subject {
      Subject::New => "task_form.kept.noteNew",
      Subject::Draft(_) => "task_form.kept.note",
      Subject::Ran(_) => "task_form.kept.noteRan",
    };
    let words = ctx.t_args(key, [("time", display::clock(at))]);
    notes.push(note("task-form-kept-note", Icon::HardDrive, &words));
  }
  notes
}

/// A note under the head (`Note` `CvMGI`): a 13 icon and 12 `text_secondary` words, 8 apart, 8 over and under and 12
/// at the sides, on `surface_raised`, radius 6.
fn note(id: &str, glyph: Icon, words: &str) -> Element {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .rounded(RadiusSize::Md)
    .child(icon(glyph, IconSize::Base, PaletteColor::TextSecondary))
    .child(
      Text::new(words)
        .id(format!("{id}-text"))
        .variant(TypographyStyle::Description)
        .flex(1.0),
    )
    .into()
}

/// A queued task opened again without a run (`Reopened later` in `t0fDw`): no run is recorded, and
/// Retry run starts it.
fn no_run_notice(ctx: &Ctx, view: &View) -> Option<Element> {
  let Phase::NotStarted { queued, problems } = &view.phase else {
    return None;
  };
  (view.subject.awaits_first_run() && problems.is_empty()).then(|| {
    Notice::new(
      "task-form-no-run",
      NoticeTone::Info,
      ctx.t("task_form.queued.noRun.title"),
    )
    .icon(Icon::CirclePause)
    .body(ctx.t_args("task_form.queued.noRun.body", [("key", queued.key.to_string())]))
    .into()
  })
}

/// What the form holds and does, at the head's end (`Status` `cBsTM`).
fn save_status(ctx: &Ctx, view: &View) -> Row {
  let (glyph, color, words) = status(ctx, view);
  Row::new()
    .id("task-form-status")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .flex_shrink(0.0)
    .describe("status", words.clone())
    .child(icon(glyph, IconSize::Sm, color.clone()))
    .child(
      Text::new(&words)
        .id("task-form-status-label")
        .variant(TypographyStyle::Description)
        .color(color)
        .nowrap(),
    )
}

fn status(ctx: &Ctx, view: &View) -> (Icon, PaletteColor, Arc<str>) {
  let doing = |key| (Icon::LoaderCircle, PaletteColor::TextSecondary, ctx.t(key));
  match &view.phase {
    Phase::Saving if view.subject.is_new() => doing("task_form.status.creating"),
    Phase::Saving => doing("task_form.status.saving"),
    Phase::Creating => doing("task_form.status.creating"),
    Phase::Starting(_) => doing("task_form.status.starting"),
    Phase::NotStarted { queued, .. } => match queued.outcome {
      StartOutcome::ConfirmedNotStarted => (
        Icon::CirclePause,
        PaletteColor::Warning,
        ctx.t("task_form.status.notStarted"),
      ),
      StartOutcome::Unconfirmed => (
        Icon::CircleHelp,
        PaletteColor::Warning,
        ctx.t("task_form.status.unconfirmed"),
      ),
    },
    Phase::Editing => editing_status(ctx, view),
  }
}

fn editing_status(ctx: &Ctx, view: &View) -> (Icon, PaletteColor, Arc<str>) {
  if view.newer || view.conflict.is_some() {
    return (Icon::GitCompare, PaletteColor::Warning, ctx.t("task_form.status.newer"));
  }
  if let Some(at) = view.kept_at.filter(|_| view.changed) {
    let words = ctx.t_args("task_form.status.kept", [("time", display::clock(at))]);
    return (Icon::CircleDot, PaletteColor::Warning, words);
  }
  match (&view.subject, view.changed) {
    (Subject::New, _) => (
      Icon::CircleDashed,
      PaletteColor::TextMuted,
      ctx.t("task_form.status.notSaved"),
    ),
    (_, true) => (
      Icon::CircleDot,
      PaletteColor::Warning,
      ctx.t("task_form.status.unsaved"),
    ),
    (Subject::Draft(_), false) => (Icon::Check, PaletteColor::TextMuted, ctx.t("task_form.status.saved")),
    (Subject::Ran(_), false) => (Icon::Check, PaletteColor::TextMuted, ctx.t("task_form.status.savedRan")),
  }
}
