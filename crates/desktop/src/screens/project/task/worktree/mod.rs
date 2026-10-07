//! *Remove worktree* for a run with no live work (it ended, or it was interrupted) that still has
//! worktrees in `.orchester/worktrees/`: its own and those of steps that ran beside others and were
//! kept. For the task's latest run the button sits in the topbar where the run controls are while a
//! run works (`ctwcG`, 02.P `GIiCb`); every run's row in *Runs of this task* has one too (ORC-63,
//! 63.01 `O0AMKY`). The confirmation follows the dialogs of the app (`jYL1V`; 02.Q `LMrbK`, 63.03
//! `e4QBB`, 63.04 `rkAU3`, see [`dialog`]). Git removes the worktrees, build output included;
//! committed work stays on the branches, and only branches whose commits are merged are deleted. A
//! worktree with uncommitted changes is removed only after the person switched *Discard the
//! uncommitted changes listed above* on, and only with exactly the changes the dialog listed. One
//! removal runs at a time per task: while one runs, the task's other buttons wait.

mod dialog;
mod read;

use std::sync::Arc;

use lurq::{
  app::{component::Component, ctx::Ctx},
  components::{Column, Modal, ModalTarget, Row},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{RunId, TaskId},
  integration::Leftovers,
  run::{Run, RunNumber},
};

use self::dialog::{Dialog, dialog};
pub(super) use self::read::{TaskLeftovers, task_leftovers};
use super::super::{ProjectNav, page};
use crate::{
  activity::Activity,
  programs::Programs,
  tasks::{
    detail::TaskDetail,
    integration_jobs::{JobState, RemoveArgs, Remover, RunIntegration},
  },
  ui::{button::ActionButton, icon::Icon},
  workspace::OpenProject,
};

/// The button in a run's row (`WTe9u` in 63.01): `u1XBP` at 32.
const ROW_BUTTON_HEIGHT: f32 = 32.0;

/// Where the button is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Place {
  /// The topbar of the task's latest run; it shows the confirmation.
  Topbar,
  /// A run's row in *Runs of this task*; `dialog`: it shows the confirmation, which the topbar
  /// does for the latest run.
  Row { dialog: bool },
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RemoveWorktreeProps {
  pub(super) open: OpenProject,
  pub(super) task: TaskId,
  /// The task's key, as the status bar names the operation.
  pub(super) key: Arc<str>,
  /// The run, which has no live work.
  pub(super) run: Arc<Run>,
  /// What the run left, with at least one worktree.
  pub(super) leftovers: Leftovers,
  pub(super) place: Place,
  /// The task's other runs: while one of them is being removed or merged, this one waits.
  pub(super) others: Arc<[RunId]>,
  /// Another run of the task at work, whose worktree is not touched.
  pub(super) working: Option<RunNumber>,
}

impl lurq::app::component::DevtoolsInspectable for RemoveWorktreeProps {}

impl RemoveWorktreeProps {
  /// The topbar's *Remove worktree* for the latest run of `detail`, when it has no live work and
  /// still has worktrees as `left` says, or its dialog shows them removed elsewhere ([`listed`]).
  pub(super) fn topbar(
    (open, nav): (&OpenProject, &ProjectNav),
    detail: &TaskDetail,
    left: Option<&TaskLeftovers>,
  ) -> Option<Self> {
    let latest = detail.run.as_ref()?.id();
    let record = detail.history.iter().find(|record| record.id == latest)?;
    let run = record.without_live_work()?;
    let leftovers = listed(nav, latest, left?.of(latest)?.ok()?)?;
    Some(Self {
      open: open.clone(),
      task: detail.task.id(),
      key: detail.task.key().to_string().into(),
      run: run.clone(),
      leftovers,
      place: Place::Topbar,
      others: others(detail, latest),
      working: working(detail),
    })
  }
}

/// What the removal of `run` shows: `left` while it has worktrees; once they were removed elsewhere,
/// e.g. by automation, while the run's dialog was open, nothing, and the dialog stays open and says
/// so. `None` when there is nothing to show.
pub(super) fn listed(nav: &ProjectNav, run: RunId, left: Leftovers) -> Option<Leftovers> {
  if !left.worktrees.is_empty() {
    return Some(left);
  }
  let job = nav.integration(run);
  (job.dialog_open().get() && job.remove().get() == JobState::Succeeded).then_some(left)
}

/// The runs of `detail` other than `run`.
pub(super) fn others(detail: &TaskDetail, run: RunId) -> Arc<[RunId]> {
  detail
    .history
    .iter()
    .map(|record| record.id)
    .filter(|id| *id != run)
    .collect()
}

/// The run of `detail` an agent works for, if any: its number.
pub(super) fn working(detail: &TaskDetail) -> Option<RunNumber> {
  detail
    .history
    .iter()
    .filter_map(|record| record.run.as_ref())
    .find(|run| run.status().has_live_work())
    .map(|run| run.number())
}

pub(super) struct RemoveWorktree {
  nav: ProjectNav,
}

impl Component for RemoveWorktree {
  type Props = RemoveWorktreeProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      nav: ctx
        .use_context::<ProjectNav>()
        .expect("the project shell provides ProjectNav"),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RemoveWorktreeProps>().clone();
    let job = self.nav.integration(props.run.id());
    let shows_dialog = matches!(props.place, Place::Topbar | Place::Row { dialog: true });
    // Removed elsewhere while its dialog is open: only the dialog stays, saying so.
    if props.leftovers.worktrees.is_empty() {
      return Row::new().with_children(shows_dialog.then(|| modal(ctx, &props, &job)));
    }
    let waits = self.waits(&props.others);
    let removing = matches!(job.remove().get(), JobState::Running);
    let busy = job.busy()
      || waits
      || matches!(job.merge().get(), JobState::Running)
      || matches!(job.remove().get(), JobState::Running | JobState::Succeeded);
    let several = props.leftovers.worktrees.len() > 1;
    let (label, glyph, height) = match props.place {
      Place::Row { .. } if removing => ("runs.worktree.removing", Icon::Loader, ROW_BUTTON_HEIGHT),
      Place::Row { .. } => (remove_key(several), Icon::FolderX, ROW_BUTTON_HEIGHT),
      Place::Topbar => (remove_key(several), Icon::FolderX, page::TOPBAR_BUTTON_HEIGHT),
    };
    let id = match props.place {
      Place::Topbar => "task-remove-worktree".to_owned(),
      Place::Row { .. } => format!("run-remove-worktree-{}", props.run.number().get()),
    };
    let (nav, others) = (self.nav.clone(), props.others.clone());
    let open = job.clone();
    let button = ActionButton::new(ctx.t(label))
      .icon(glyph)
      .id(id)
      .height(Dimension::Px(height))
      .on_click(move || {
        let waits = others.iter().any(|run| nav.integration(*run).busy());
        if open.busy() || waits || matches!(open.remove().get_untracked(), JobState::Succeeded) {
          return;
        }
        open.open_remove();
      })
      .enabled(!busy)
      .render(ctx);
    Row::new()
      .child(button)
      .with_children(shows_dialog.then(|| modal(ctx, &props, &job)))
  }
}

impl RemoveWorktree {
  /// Whether another run of the task is being removed or merged: one removal at a time.
  fn waits(&self, others: &[RunId]) -> bool {
    others.iter().any(|run| self.nav.integration(*run).busy())
  }
}

const fn remove_key(several: bool) -> &'static str {
  if several {
    "runs.worktree.remove.other"
  } else {
    "runs.worktree.remove.one"
  }
}

fn modal(ctx: &mut Ctx, props: &RemoveWorktreeProps, job: &RunIntegration) -> Element {
  if !job.dialog_open().get() {
    return Modal::new(Column::new()).open_when(false).into();
  }
  let state = job.remove().get();
  let pending = matches!(state, JobState::Running);
  let busy = job.busy() || matches!(job.merge().get(), JobState::Running);
  let loses_files = props.leftovers.loses_files();
  let confirm = (!busy && !pending && (!loses_files || job.discard().get())).then(|| {
    let args = remove_args(ctx, props);
    let job = job.clone();
    Arc::new(move || job.start_remove(args.clone())) as Arc<dyn Fn() + Send + Sync>
  });
  let discard = job.discard();
  let close = job.clone();
  let content = dialog(
    ctx,
    &Dialog {
      props,
      discard: job.discard().get(),
      flip_discard: Arc::new(move || discard.update(|on| *on = !*on)),
      pending,
      error: match state {
        JobState::Failed(error) => Some(error),
        _ => None,
      },
      confirm,
      cancel: Arc::new(move || close.dismiss_remove()),
    },
  );
  Modal::new(content)
    .target(ModalTarget::Root)
    .dismiss_on_escape(false)
    .into()
}

/// The exact worktrees and uncommitted files that the confirmation listed.
fn remove_args(ctx: &mut Ctx, props: &RemoveWorktreeProps) -> RemoveArgs {
  let programs = ctx
    .use_context::<Programs>()
    .expect("the root provides Programs before mounting screens");
  let activity = Activity::of(ctx);
  let separator = ctx.t("common.listSeparator");
  let nav = ctx
    .use_context::<ProjectNav>()
    .expect("the project shell provides ProjectNav");
  RemoveArgs {
    open: props.open.clone(),
    task: props.task,
    key: props.key.to_string(),
    number: props.run.number().get(),
    run: props.run.as_ref().clone(),
    confirmed: props.leftovers.clone(),
    programs,
    activity,
    separator,
    changed: Arc::new(move || nav.tasks_changed()),
    by: Remover::Person,
  }
}
