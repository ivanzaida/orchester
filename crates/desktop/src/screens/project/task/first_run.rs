//! *Retry run* in the topbar of a queued task that has no run recorded (`docs/design/orchester-workspace.md`,
//! "A queued task whose run did not start"): when this window's New task form did not queue it, e.g.
//! automation queued it through Orchester's MCP server, the task opens on its page, which reads *Queued
//! · its run has not started*, rather than in the locked form (02.09F). Retry asks the runner to start
//! the task's first run under the task's first-run key, as the form's Retry run does, so it can never
//! start a second run. A shared task's work starts only under the person's claim.

use lurq::{
  app::ctx::{Ctx, FutureAction},
  node::{Element, dimension::Dimension},
};
use orchester_core::{id::TaskId, task::TaskStatus};

use super::{super::page, TaskProps};
use crate::{
  messages::Message,
  runner::Runners,
  tasks::{detail::TaskDetail, run::control_message},
  ui::{button::ActionButton, icon::Icon},
};

pub(super) type FirstRunAction = FutureAction<TaskId, (), Message>;

pub(super) fn first_run_action(ctx: &mut Ctx) -> FirstRunAction {
  let runners = ctx
    .use_context::<Runners>()
    .expect("the root provides Runners before mounting screens");
  let servers = crate::sharing::feed::ServerFeed::of(ctx).servers;
  let project = ctx.props::<TaskProps>().project.clone();
  ctx.future_action(move |task: TaskId| {
    let (runners, servers, project) = (runners.clone(), servers.clone(), project.clone());
    async move {
      let claim = servers
        .claim_for_work(&project.repo_root, task)
        .await
        .map_err(|refusal| refusal.message())?;
      let started = runners.start_run_in(&project, task.first_run_key(), task).await;
      if started.is_err() {
        claim.release(&servers).await;
      }
      started.map(|_| ()).map_err(|error| {
        tracing::warn!(%error, "the runner did not start the queued task's first run");
        control_message(&error)
      })
    }
  })
}

/// *Retry run* for a queued task without a run; unavailable while the request is under way or while
/// unfinished tasks block it, which the page's Blocked notice says.
pub(super) fn retry_button(ctx: &Ctx, detail: &TaskDetail, action: &FirstRunAction) -> Option<Element> {
  let task = &detail.task;
  if task.status() != TaskStatus::Queued || detail.run.is_some() || detail.latest_unreadable.is_some() {
    return None;
  }
  let button = if action.state().get().is_pending() {
    ActionButton::new(ctx.t("runs.page.starting"))
      .icon(Icon::Loader)
      .primary()
      .disabled()
  } else {
    let (action, id) = (action.clone(), task.id());
    ActionButton::new(ctx.t("tasks.new.retryRun"))
      .icon(Icon::Play)
      .primary()
      .on_click(move || action.run(id))
      .enabled(detail.blocked_by.is_empty())
  };
  Some(
    button
      .id("task-retry-first-run")
      .height(Dimension::Px(page::TOPBAR_BUTTON_HEIGHT))
      .render(ctx),
  )
}
