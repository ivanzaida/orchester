//! The page head (`Page head` `gfHep`): where the form is (Tasks › New task, Tasks › KON-7 › Edit), its
//! title and what it does, and at the end the top status: what the form holds and what it is doing,
//! an icon and its words in 12/500 (`Top status` `TwmHc`; the states of `J2QjVq`, `H8GYVZ`, `t0fDw`,
//! `Qp1B0`, `v2c76`, `Fd1ai`). Only what happened is said: a task is "queued" once it is, a run
//! "starting" while the runner is asked.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::{display, tasks::run::StartOutcome};

use super::{
  Nav,
  state::{Phase, Subject, View},
};
use crate::{
  theme::{ControlHeight, ExtraSpacing, IconSize},
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    icon::{Icon, icon},
    notice::{Notice, NoticeTone},
  },
};

pub fn head(ctx: &mut Ctx, view: &View, nav: &Nav) -> Element {
  let (title, description) = texts(ctx, view);
  let text = Column::new()
    .flex(1.0)
    .spacing(SpacingSize::Xs)
    .child(
      Text::new(&title)
        .id("task-form-title")
        .variant(TypographyStyle::Heading),
    )
    .child(
      Text::new(&description)
        .id("task-form-description")
        .variant(TypographyStyle::Description),
    );
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(crumbs(ctx, view, nav))
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Lg)
        .align_items(Alignment::Start)
        .child(text)
        .child(top_status(ctx, view)),
    )
    .into()
}

fn crumbs(ctx: &mut Ctx, view: &View, nav: &Nav) -> Row {
  let tasks = {
    let nav = nav.clone();
    Crumb::link(ctx.t("task_form.crumbs.tasks"), move || nav.tasks(None))
  };
  let crumbs = match view.subject.task() {
    None => vec![tasks, Crumb::here(ctx.t("task_form.crumbs.newTask"))],
    Some(task) if view.subject.awaits_first_run() => vec![tasks, Crumb::here(task.key().to_string()).mono()],
    Some(task) => {
      let nav = nav.clone();
      let key = task.key().clone();
      vec![
        tasks,
        Crumb::link(key.to_string(), move || nav.task(&key)).mono(),
        Crumb::here(ctx.t("task_form.crumbs.edit")),
      ]
    }
  };
  breadcrumb(ctx, "task-form-crumbs", CrumbSeparator::Chevron, crumbs)
}

/// The page's title and what it says under it.
fn texts(ctx: &Ctx, view: &View) -> (Arc<str>, Arc<str>) {
  let queued = match &view.phase {
    Phase::Starting(queued) => Some(("task_form.queued.savedAs", queued)),
    Phase::NotStarted { queued, .. } => Some((
      match queued.outcome {
        StartOutcome::ConfirmedNotStarted => "task_form.queued.description",
        StartOutcome::Unconfirmed => "task_form.queued.unconfirmedDescription",
      },
      queued,
    )),
    Phase::Editing | Phase::Saving | Phase::Creating => None,
  };
  if let Some((key, queued)) = queued {
    let title = ctx.t("task_form.new.title");
    return (title, ctx.t_args(key, [("key", queued.key.to_string())]));
  }
  match &view.subject {
    Subject::New => (ctx.t("task_form.new.title"), ctx.t("task_form.new.description")),
    Subject::Draft(task) => (
      ctx.t("task_form.edit.title"),
      ctx.t_args("task_form.edit.description", [("key", task.key().to_string())]),
    ),
    Subject::Ran(task) => (
      ctx.t("task_form.edit.title"),
      ctx.t_args("task_form.edit.descriptionRan", [("key", task.key().to_string())]),
    ),
  }
}

/// A queued task opened again without a run (`Reopened later` in `t0fDw`): no run is recorded, and
/// Retry run starts it.
pub fn no_run_notice(ctx: &Ctx, view: &View) -> Option<Element> {
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

/// What the form holds and does, at the head's end (`Top status` `TwmHc`).
fn top_status(ctx: &Ctx, view: &View) -> Row {
  let (glyph, color, words) = status(ctx, view);
  Row::new()
    .id("task-form-status")
    .height(Dimension::Px(ControlHeight::Md.value()))
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .flex_shrink(0.0)
    .describe("status", words.clone())
    .child(icon(glyph, IconSize::Base, color.clone()))
    .child(
      Text::new(&words)
        .id("task-form-status-label")
        .variant(TypographyStyle::Label)
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
  if let Some(at) = view.kept_at.filter(|_| view.changed) {
    let words = ctx.t_args("task_form.status.kept", [("time", display::clock(at))]);
    return (Icon::CircleDot, PaletteColor::TextSecondary, words);
  }
  match (&view.subject, view.changed) {
    (Subject::New, _) => (
      Icon::CircleDashed,
      PaletteColor::TextMuted,
      ctx.t("task_form.status.notSaved"),
    ),
    (_, true) => (
      Icon::CircleDot,
      PaletteColor::TextSecondary,
      ctx.t("task_form.status.unsaved"),
    ),
    (Subject::Draft(_), false) => (Icon::Check, PaletteColor::TextMuted, ctx.t("task_form.status.saved")),
    (Subject::Ran(_), false) => (Icon::Check, PaletteColor::TextMuted, ctx.t("task_form.status.savedRan")),
  }
}
