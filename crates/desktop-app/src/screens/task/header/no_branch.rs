//! A run the runner refused for want of a branch (`Base branch · redesign` `ri5ru`, `Refused start` `J18QL` in
//! `PgNtj`): neither the task nor its module has a base branch and the checkout has a detached HEAD, so the run has no
//! branch to deliver into. The header's status line says so in place of the run's status, a 14 `triangle-alert` and
//! *Can't start: no branch checked out* in 13/500 `warning`, then why in 12 `text_muted`, 8 apart, with *Set base
//! branch* (Edit task, where the Base branch chip is) and *Run again* under it, 8 apart; no card. It stays until the
//! task's latest run changes or Run again is pressed.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{IdempotencyKey, RunId},
  integration::base::TaskBase,
  run::PreflightProblem,
};
use orchester_ipc::Rejection;
use orchester_workspace::{display, runner::RunRequestError};

use super::run_again::Confirming;
use crate::{
  navigation::{Navigation, press::OpenIn},
  routes::{ProjectPage, Route},
  screens::task::TaskContext,
  theme::{ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

/// A start of the task the runner refused for want of a branch: the run after `after` (none for a first run), from a
/// checkout detached at `head`, as git names it short.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoBranch {
  pub head: Arc<str>,
  pub after: Option<RunId>,
}

impl lurq::app::component::DevtoolsInspectable for NoBranch {}

impl NoBranch {
  /// The refusal `error` holds, when the runner refused the run after `after` for want of a branch.
  pub fn of(error: &RunRequestError, after: Option<RunId>) -> Option<Self> {
    let RunRequestError::Rejected(Rejection::Preflight { problems }) = error else {
      return None;
    };
    problems.iter().find_map(|problem| match problem {
      PreflightProblem::NoBranchCheckedOut { head } => Some(Self {
        head: display::short_revision(head.as_str()).into(),
        after,
      }),
      _ => None,
    })
  }

  /// The refusal `refused` holds while it is about the task's latest run as `task` shows it.
  pub fn shown(refused: Option<Self>, task: &TaskContext) -> Option<Self> {
    refused.filter(|refused| refused.after == task.detail.run.as_ref().map(|run| run.id()))
  }
}

/// The status line and its actions, while `refused` holds for the task's latest run.
pub fn status_line(
  ctx: &mut Ctx,
  task: &TaskContext,
  refused: &Signal<Option<NoBranch>>,
  (confirming, remove): (&Signal<Confirming>, &Signal<bool>),
) -> Option<Element> {
  NoBranch::shown(refused.get(), task)?;
  let line = Row::new()
    .id("task-no-branch")
    .width(Dimension::Pct(100.0))
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::TriangleAlert, IconSize::Md, PaletteColor::Warning))
    .child(
      Text::new(&ctx.t("task.noBranch.title"))
        .id("task-no-branch-title")
        .variant(ExtraTypography::Strong.role())
        .color(PaletteColor::Warning)
        .nowrap(),
    )
    .child(
      Text::new(&why(ctx, task))
        .id("task-no-branch-why")
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    );
  Some(
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(line)
      .child(actions(ctx, task, refused, (confirming, remove)))
      .into(),
  )
}

/// Why the run had no branch: the task inherits from a module that sets none, is in no module, or takes the checked-out
/// branch.
fn why(ctx: &Ctx, task: &TaskContext) -> Arc<str> {
  let detail = &task.detail.task;
  let key = match (detail.base(), detail.module()) {
    (TaskBase::CheckedOut, _) => "task.noBranch.takesCheckedOut",
    (_, Some(_)) => "task.noBranch.moduleNone",
    (_, None) => "task.noBranch.noModule",
  };
  ctx.t_args(key, [("task", detail.key().to_string())])
}

/// *Set base branch*, which opens Edit task, and *Run again*, which opens Run again's confirmation.
fn actions(
  ctx: &mut Ctx,
  task: &TaskContext,
  refused: &Signal<Option<NoBranch>>,
  (confirming, remove): (&Signal<Confirming>, &Signal<bool>),
) -> Element {
  let navigation = Navigation::of(ctx);
  let edit = Route::Project(
    task.project.open.project.id(),
    ProjectPage::EditTask(task.detail.task.key().clone()),
  );
  let set = ActionButton::new(
    "task-no-branch-set",
    ButtonKind::Secondary,
    ctx.t("task.noBranch.setBaseBranch"),
  )
  .icon(Icon::GitBranch)
  .on_click(move || navigation.open(edit.clone(), OpenIn::ActiveTab));
  let (refused, confirming, remove) = (refused.clone(), confirming.clone(), remove.clone());
  let again = ActionButton::new(
    "task-no-branch-run-again",
    ButtonKind::Ghost,
    ctx.t("task.runAgain.action"),
  )
  .icon(Icon::RotateCcw)
  .on_click(move || {
    refused.set(None);
    remove.set(false);
    confirming.set(Confirming(Some(IdempotencyKey::generate())));
  });
  Row::new()
    .id("task-no-branch-actions")
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(set)
    .child(again)
    .into()
}
