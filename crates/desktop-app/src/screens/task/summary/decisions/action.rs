//! A decision goes to the project's runner, which records it and carries it out: approving or asking for
//! changes to a plan, raising a limit, allowing a retry, returning the work or stopping the run. While
//! its request is under way the decision's buttons wait; what it decides leaves every *Needs you* in the
//! press's frame; the runner's refusal brings it back and is said where the person decided. A runner that
//! is not connected refuses too, in so many words.

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureAction},
};
use orchester_ipc::Command;
use orchester_workspace::{messages::Message, tasks::run::control_message, workspace::OpenProject};

use crate::{project::Projects, services::Services};

/// A request to the runner, `T` naming which decision it carries so its button can show it under way.
pub type DecisionAction<T> = FutureAction<(T, Command), T, Message>;

/// Sends a decision's command to the runner of `project`.
pub fn decision_action<T>(ctx: &mut Ctx, project: OpenProject) -> DecisionAction<T>
where
  T: Clone + PartialEq + Send + Sync + DevtoolsInspectable + 'static,
{
  let runners = Services::of(ctx).runners;
  let feed = Projects::of(ctx).feed(project.project.id());
  ctx.future_action(move |(choice, command): (T, Command)| {
    let sent = runners.control_run_in(&project, command);
    feed.answer_sent();
    async move {
      sent.await.map(|()| choice).map_err(|error| {
        tracing::warn!(%error, "the runner did not take the decision");
        control_message(&error)
      })
    }
  })
}

/// The decision under way, if any, and why the last one was refused.
pub fn state<T: Clone + PartialEq + Send + Sync + DevtoolsInspectable + 'static>(
  action: &DecisionAction<T>,
  asked: Option<T>,
) -> (Option<T>, Option<Message>) {
  let state = action.state().get();
  if state.is_pending() {
    return (asked, None);
  }
  (None, state.error)
}
