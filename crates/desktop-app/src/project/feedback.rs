//! Held agent feedback goes to its task's agent as soon as the run's state lets it (`orchester_workspace::
//! pull_requests::feedback::dispatch_in`): whenever the project's runs change state or its runner reports another use
//! of its agent slots, the held requests are looked at again and those that may go are sent, the runner told of their
//! runs. Mounted with the project's feed, so it follows the project while its tabs are inactive and whether or not a
//! pull request's page is open.

use std::path::Path;

use lurq::app::ctx::Ctx;
use orchester_workspace::{
  background,
  messages::Message,
  pull_requests::feedback::{Slots, dispatch_in, tell_runner},
  runner::{RunnerStatus, state::RunnerState},
  workspace::OpenProject,
};

use crate::services::Services;

/// What the hold rules of held feedback know of the runner of the project at `root`, which reported `status`: its
/// agent slots and how many work, and whether it is gone. Every send of held feedback passes these, so a follow-up
/// waits for a slot and a run without its runner takes no message, wherever the send starts.
pub fn feedback_holds(status: Option<&RunnerStatus>, root: &Path) -> (Option<Slots>, bool) {
  let slots = status.and_then(|status| status.work().map(Slots::of));
  let lost = status.is_none_or(|status| status.state_for(root) != RunnerState::Online);
  (slots, lost)
}

/// Sends `open`'s held feedback that may go, whenever its run `states` or its runner's `status` change. Renders
/// nothing.
pub(super) fn follow(ctx: &mut Ctx, (open, states): (&OpenProject, u64), status: Option<&RunnerStatus>) {
  let services = Services::of(ctx);
  let (slots, lost) = feedback_holds(status, &open.repo_root);
  let open = open.clone();
  let deps = (
    open.repo_root.clone(),
    states,
    slots.map(|slots| (slots.working, slots.slots)),
    lost,
  );
  ctx.future(deps, move |_| {
    let (services, open) = (services.clone(), open.clone());
    async move {
      let root = open.repo_root.clone();
      let dispatched = background::run(move || dispatch_in(&root, slots, lost))
        .await?
        .map_err(|error| {
          tracing::warn!(%error, "cannot send the project's held feedback");
          error.message()
        })?;
      tell_runner(&services.runners, &services.servers, &open, &dispatched).await;
      Ok::<(), Message>(())
    }
  });
}
