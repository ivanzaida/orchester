//! *Remove worktree* for a run that ended (succeeded, failed or cancelled) and still has worktrees
//! in `.orchester/worktrees/`: its own and those of steps that ran beside others and were kept. The
//! button sits in the topbar where the run controls are while a run works (`ctwcG`, 02.P `GIiCb`),
//! and the confirmation follows the dialogs of the app (`jYL1V`; 02.Q `LMrbK`, see [`dialog`]). Git
//! removes the worktrees, build output included; committed work stays on the branches, and only
//! branches whose commits are merged are deleted. A worktree with uncommitted changes is removed
//! only after the person switched *Discard the uncommitted changes listed above* on, and only with
//! exactly the changes the dialog listed.

mod dialog;

use std::sync::Arc;

use lurq::{
  app::{component::Component, ctx::Ctx},
  components::{Column, Modal, ModalTarget, Row},
  node::{Element, dimension::Dimension},
};
use orchester_core::integration::Leftovers;

use self::dialog::{Dialog, dialog};
use super::super::{ProjectNav, page};
use crate::{
  activity::Activity,
  programs::Programs,
  tasks::{
    detail::TaskDetail,
    integration_jobs::{JobState, RemoveArgs, RunIntegration},
  },
  ui::{button::ActionButton, icon::Icon},
  workspace::OpenProject,
};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RemoveWorktreeProps {
  pub(super) open: OpenProject,
  /// The task, whose latest run ended.
  pub(super) detail: Arc<TaskDetail>,
  /// What the run left, with at least one worktree.
  pub(super) leftovers: Leftovers,
}

impl lurq::app::component::DevtoolsInspectable for RemoveWorktreeProps {}

pub(super) struct RemoveWorktree;

impl Component for RemoveWorktree {
  type Props = RemoveWorktreeProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RemoveWorktreeProps>().clone();
    let job = ctx
      .use_context::<RunIntegration>()
      .expect("the task page provides its run integration");
    let busy = job.busy()
      || matches!(job.merge().get(), JobState::Running)
      || matches!(job.remove().get(), JobState::Running | JobState::Succeeded);
    let (open, discard) = (job.dialog_open(), job.discard());
    let key = if props.leftovers.worktrees.len() == 1 {
      "runs.worktree.remove.one"
    } else {
      "runs.worktree.remove.other"
    };
    let button = ActionButton::new(ctx.t(key))
      .icon(Icon::FolderX)
      .id("task-remove-worktree")
      .height(Dimension::Px(page::TOPBAR_BUTTON_HEIGHT))
      .on_click(move || {
        if job.busy() || matches!(job.remove().get_untracked(), JobState::Succeeded) {
          return;
        }
        discard.set(false);
        open.set(true);
      })
      .enabled(!busy)
      .render(ctx);
    Row::new().child(button).child(self.modal(ctx, &props))
  }
}

impl RemoveWorktree {
  fn modal(&self, ctx: &mut Ctx, props: &RemoveWorktreeProps) -> Element {
    let job = ctx
      .use_context::<RunIntegration>()
      .expect("the task page provides its run integration");
    if !job.dialog_open().get() {
      return Modal::new(Column::new()).open_when(false).into();
    }
    let state = job.remove().get();
    let pending = matches!(state, JobState::Running);
    let busy = job.busy() || matches!(job.merge().get(), JobState::Running);
    let uncommitted = props.leftovers.uncommitted() > 0;
    let confirm = (!busy && !pending && (!uncommitted || job.discard().get())).then(|| {
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
  let run = props
    .detail
    .run
    .as_ref()
    .map(|run| run.run.clone())
    .expect("a run's worktrees are removed from its task page");
  RemoveArgs {
    open: props.open.clone(),
    task: props.detail.task.id(),
    key: props.detail.task.key().to_string(),
    number: run.number().get(),
    run,
    confirmed: props.leftovers.clone(),
    programs,
    activity,
    separator,
    changed: Arc::new(move || nav.tasks_changed()),
  }
}
