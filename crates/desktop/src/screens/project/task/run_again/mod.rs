//! *Run again* (ORC-16): for a task whose latest run failed or was cancelled, or was interrupted and
//! lost its worktree (ORC-63, 63.05 `LmTnc`), the primary action at
//! the right of the topbar (rotate-ccw, `zM693`, `M1upHe`), which opens the confirmation
//! ([`dialog`], `R5y5x`) and asks the runner to start the task's next run. While core's rule says
//! the task cannot run again, it stays in its place unavailable (0.4, trailing lock) and the lock
//! line under the run's notice says why ([`Offer::reason`], `iE8eQ`). One request per confirmation: its
//! idempotency key is made when the dialog opens, so pressing Start again, or retrying after a lost
//! reply, never starts a second run.

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
use orchester_core::{
  id::{IdempotencyKey, RunId, TaskId},
  integration::Leftovers,
  run::can_run_again_after,
};

use self::dialog::{Dialog, dialog};
use super::super::{ProjectNav, page};
use crate::{
  background,
  messages::Message,
  programs::Programs,
  runner::Runners,
  tasks::{
    detail::TaskDetail,
    integration_jobs::RunIntegration,
    run_again::{RunAgainCheck, block_message, request_messages},
  },
  ui::{button::ActionButton, icon::Icon, message_text, notice::lock_line},
  workspace::OpenProject,
};

/// What the task page shows of Run again: its topbar action, right of *Remove worktree*, and on
/// the Summary why the task cannot run again. Nothing for a latest run that did not fail and was
/// not cancelled, or until whether it can run again was read the first time.
#[derive(Default)]
pub(super) struct Offer {
  pub(super) action: Option<Element>,
  pub(super) reason: Option<Element>,
}

/// [`Offer`] for `detail`, whose latest run `leftovers` says what it left; read with the task and
/// again whenever it is (`revision`).
pub(super) fn offer(
  ctx: &mut Ctx,
  open: &OpenProject,
  detail: &Arc<TaskDetail>,
  revision: (u64, u64),
  leftovers: Option<&Result<Leftovers, Message>>,
) -> Offer {
  let Some(run) = detail
    .run
    .as_ref()
    .filter(|run| can_run_again_after(&run.run, run.worktree_removed.is_some()))
  else {
    return Offer::default();
  };
  let programs = ctx
    .use_context::<Programs>()
    .expect("the root provides Programs before mounting screens");
  let deps = (open.repo_root.clone(), detail.task.id(), run.id(), revision);
  let load = ctx.future(deps, move |(repo_root, task, after, _)| {
    let programs = programs.clone();
    async move {
      background::run(move || RunAgainCheck::load_in(&repo_root, &programs, task, after))
        .await?
        .map_err(|error| {
          tracing::warn!(%error, "cannot check whether the task can run again");
          crate::messages::store_error(&error)
        })
    }
  });
  let state = load.state().get();
  // A check for an earlier run of the task says nothing about this one.
  let Some(check) = state.data.filter(|check| check.after == run.id()) else {
    return match state.error {
      Some(error) => Offer {
        action: Some(button(ctx).render(ctx)),
        reason: Some(lock_line("task-run-again-reason", &message_text(ctx, &error))),
      },
      None => Offer::default(),
    };
  };
  let reason = check.next.as_ref().err().map(|block| {
    let text = message_text(ctx, &block_message(*block, &check.pipeline));
    lock_line("task-run-again-reason", &text)
  });
  // Mounted anew when it becomes unavailable or available again: a dialog left open must not
  // come back on its own.
  let action = ctx.mount_keyed::<RunAgain>(
    &format!("task-run-again-{}-{}", run.id(), check.next.is_ok()),
    RunAgainProps {
      root: open.repo_root.clone(),
      detail: detail.clone(),
      check,
      leftovers: leftovers.and_then(|left| left.as_ref().ok()).cloned(),
    },
  );
  Offer {
    action: Some(action),
    reason,
  }
}

/// *Run again*, unavailable until it is given what pressing it does.
fn button(ctx: &Ctx) -> ActionButton {
  ActionButton::new(ctx.t("runs.runAgain.action"))
    .primary()
    .icon(Icon::RotateCcw)
    .id("task-run-again")
    .height(Dimension::Px(page::TOPBAR_BUTTON_HEIGHT))
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RunAgainProps {
  root: std::path::PathBuf,
  /// The task, whose latest run failed or was cancelled.
  detail: Arc<TaskDetail>,
  check: RunAgainCheck,
  /// What that run left, once read: the confirmation can remove its worktrees with it.
  leftovers: Option<Leftovers>,
}

impl lurq::app::component::DevtoolsInspectable for RunAgainProps {}

/// What one confirmation asks the runner.
#[derive(Clone)]
struct Request {
  key: IdempotencyKey,
  task: TaskId,
  after: RunId,
  remove_worktrees: Option<Leftovers>,
  pipeline: Arc<str>,
}

/// Why the runner did not start the run the confirmation with `key` asked for.
#[derive(Clone, Debug, PartialEq)]
struct Refused {
  key: IdempotencyKey,
  messages: Vec<Message>,
}

impl lurq::app::component::DevtoolsInspectable for Refused {}

type StartAction = FutureAction<Request, (), Refused>;

/// The idempotency key of one confirmation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ConfirmationKey(IdempotencyKey);

impl lurq::app::component::DevtoolsInspectable for ConfirmationKey {}

struct RunAgain {
  open: Signal<bool>,
  /// The switch *Remove the worktree of run #N*, off each time the dialog opens.
  remove: Signal<bool>,
  /// The request's key, made anew each time the dialog opens.
  key: Signal<ConfirmationKey>,
}

impl Component for RunAgain {
  type Props = RunAgainProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      remove: ctx.signal(false),
      key: ctx.signal(ConfirmationKey(IdempotencyKey::generate())),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RunAgainProps>().clone();
    let action = self.start_action(ctx);
    let job = ctx
      .use_context::<RunIntegration>()
      .expect("the task page provides its run integration");
    let pending = action.state().get().is_pending();
    let button = if props.check.next.is_ok() {
      let (open, remove, key) = (self.open.clone(), self.remove.clone(), self.key.clone());
      button(ctx)
        .on_click(move || {
          remove.set(false);
          key.set(ConfirmationKey(IdempotencyKey::generate()));
          open.set(true);
        })
        // Git removing or merging this run's worktrees must end first.
        .enabled(!job.busy() && !pending)
    } else {
      button(ctx)
    };
    Row::new()
      .child(button.render(ctx))
      .child(self.modal(ctx, &props, &action))
  }
}

impl RunAgain {
  /// Asks the runner to run the task again; closes the dialog and reads the task again once the new
  /// run started.
  fn start_action(&self, ctx: &mut Ctx) -> StartAction {
    let runners = ctx
      .use_context::<Runners>()
      .expect("the root provides Runners before mounting screens");
    let nav = ctx
      .use_context::<ProjectNav>()
      .expect("the project shell provides ProjectNav");
    let open = self.open.clone();
    let servers = crate::sharing::feed::ServerFeed::of(ctx).servers;
    let root = ctx.props::<RunAgainProps>().root.clone();
    ctx.future_action(move |request: Request| {
      let (runners, nav, open) = (runners.clone(), nav.clone(), open.clone());
      let (servers, root) = (servers.clone(), root.clone());
      async move {
        // A shared task runs again only under the person's claim.
        servers
          .claim_for_work(&root, request.task)
          .await
          .map_err(|refusal| Refused {
            key: request.key,
            messages: vec![refusal.message()],
          })?;
        let started = runners
          .run_again(request.key, request.task, request.after, request.remove_worktrees)
          .await;
        // What the page shows changed either way: the new run, or what blocked it.
        nav.tasks_changed();
        match started {
          Ok(run) => {
            tracing::info!(%run, "the task runs again");
            open.set(false);
            Ok(())
          }
          Err(error) => {
            tracing::warn!(%error, "the runner did not run the task again");
            servers.release_unused_claim(&root, request.task).await;
            Err(Refused {
              key: request.key,
              messages: request_messages(&error, &request.pipeline),
            })
          }
        }
      }
    })
  }

  fn modal(&self, ctx: &mut Ctx, props: &RunAgainProps, action: &StartAction) -> Element {
    let (Some(run), Ok(next)) = (props.detail.run.as_ref(), props.check.next.as_ref()) else {
      return Modal::new(Column::new()).open_when(false).into();
    };
    if !self.open.get() {
      return Modal::new(Column::new()).open_when(false).into();
    }
    let state = action.state().get();
    let pending = state.is_pending();
    let remove = self.remove.get();
    let ConfirmationKey(key) = self.key.get();
    let request = Request {
      key,
      task: props.detail.task.id(),
      after: run.id(),
      remove_worktrees: props.leftovers.clone().filter(|_| remove),
      pipeline: props.check.pipeline.clone(),
    };
    let confirm = (!pending).then(|| {
      let action = action.clone();
      Arc::new(move || {
        if !action.state().get_untracked().is_pending() {
          action.run(request.clone());
        }
      }) as Arc<dyn Fn() + Send + Sync>
    });
    let (flip, close) = (self.remove.clone(), self.open.clone());
    let content = dialog(
      ctx,
      &Dialog {
        detail: &props.detail,
        run,
        pipeline: &props.check.pipeline,
        next,
        leftovers: props.leftovers.as_ref(),
        remove,
        flip_remove: Arc::new(move || flip.update(|on| *on = !*on)),
        pending,
        // A refusal of an earlier confirmation is not this one's.
        errors: state
          .error
          .filter(|refused| !pending && refused.key == key)
          .map(|refused| refused.messages)
          .unwrap_or_default(),
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
