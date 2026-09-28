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
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
  },
  components::{Column, Modal, ModalTarget, Row},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::integration::Leftovers;

use self::dialog::{Dialog, dialog};
use super::super::{ProjectNav, page};
use crate::{
  activity::{Activity, operations},
  background,
  messages::Message,
  programs::Programs,
  tasks::{
    detail::TaskDetail,
    integration::{LeftBehind, remove_in},
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

type RemoveAction = FutureAction<LeftBehind, (), Message>;

pub(super) struct RemoveWorktree {
  open: Signal<bool>,
  /// The person agreed to discard the uncommitted changes the dialog lists.
  discard: Signal<bool>,
}

impl Component for RemoveWorktree {
  type Props = RemoveWorktreeProps;

  fn create(ctx: &mut Ctx) -> Self {
    let action = remove_action(ctx);
    let this = Self {
      open: ctx.signal(false),
      discard: ctx.signal(false),
    };
    let nav = ctx
      .use_context::<ProjectNav>()
      .expect("the project shell provides ProjectNav");
    let open = this.open.clone();
    // Done: the dialog closes and the task page reads what is left again.
    ctx.watch(&action.state(), move |state| {
      if !state.is_pending() && state.data.is_some() {
        open.set(false);
        nav.tasks_changed();
      }
    });
    this
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RemoveWorktreeProps>().clone();
    let action = remove_action(ctx);
    let (open, discard) = (self.open.clone(), self.discard.clone());
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
        discard.set(false);
        open.set(true);
      })
      .render(ctx);
    Row::new().child(button).child(self.modal(ctx, &props, &action))
  }
}

impl RemoveWorktree {
  fn modal(&self, ctx: &mut Ctx, props: &RemoveWorktreeProps, action: &RemoveAction) -> Element {
    if !self.open.get() {
      return Modal::new(Column::new()).open_when(false).into();
    }
    let state = action.state().get();
    let pending = state.is_pending();
    let uncommitted = props.leftovers.uncommitted() > 0;
    let confirm = (!pending && (!uncommitted || self.discard.get())).then(|| {
      let (action, leftovers) = (action.clone(), props.leftovers.clone());
      Arc::new(move || action.run(LeftBehind(leftovers.clone()))) as Arc<dyn Fn() + Send + Sync>
    });
    let (close, discard) = (self.open.clone(), self.discard.clone());
    let content = dialog(
      ctx,
      &Dialog {
        props,
        discard: self.discard.get(),
        flip_discard: Arc::new(move || discard.update(|on| *on = !*on)),
        pending,
        error: state.error.filter(|_| !pending),
        confirm,
        cancel: Arc::new(move || close.set(false)),
      },
    );
    Modal::new(content)
      .target(ModalTarget::Root)
      .dismiss_on_escape(false)
      .into()
  }
}

/// Removes what the run left, as the dialog listed it, tracked in the status bar.
fn remove_action(ctx: &mut Ctx) -> RemoveAction {
  let props = ctx.props::<RemoveWorktreeProps>().clone();
  let programs = ctx
    .use_context::<Programs>()
    .expect("the root provides Programs before mounting screens");
  let activity = Activity::of(ctx);
  let separator = ctx.t("common.listSeparator");
  ctx.future_action(move |LeftBehind(confirmed): LeftBehind| {
    let (programs, activity, separator, props) = (programs.clone(), activity.clone(), separator.clone(), props.clone());
    async move {
      let run = props
        .detail
        .run
        .as_ref()
        .map(|run| run.run.clone())
        .expect("a run's worktrees are removed from its task page");
      let task = &props.detail.task;
      let (key, number) = (task.key().to_string(), run.number().get());
      let running = activity.start(operations::run_cleanup(&props.open, task.id(), (&key, number)));
      let not_done = Message::new("statusBar.operations.runCleanup.notDone")
        .with("task", key)
        .with("run", number.to_string());
      let repo_root = props.open.repo_root.clone();
      let removed = background::run(move || remove_in(&repo_root, &programs, &run, &confirmed)).await;
      match &removed {
        Ok(Ok(())) => running.finish(),
        Ok(Err(problem)) => problem.end(running, not_done),
        Err(message) => running.fail(message.clone()),
      }
      removed?.map_err(|problem| problem.message(&separator))
    }
  })
}
