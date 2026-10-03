//! A person's change to what runs keep of their approvals (auto-approval on or off, a revoked rule),
//! written to the project database off the UI thread. Once written, every tab of the project reads its
//! tasks again, so the status bar, the task page and the dialog show the change.

use std::path::PathBuf;

use lurq::app::ctx::{Ctx, FutureAction};
use orchester_core::id::RunId;

use super::ProjectNav;
use crate::{
  background,
  messages::{self, Message},
  tasks::run_approvals::{RunApprovalChange, apply_in},
};

/// Changes of runs of the project at a repository root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RunApprovalChanges {
  pub repo_root: PathBuf,
  pub changes: Vec<(RunId, RunApprovalChange)>,
}

impl lurq::app::component::DevtoolsInspectable for RunApprovalChanges {}

pub(crate) type RunApprovalAction = FutureAction<RunApprovalChanges, (), Message>;

/// The action that writes changes; matched by call order, like every future of a component.
pub(crate) fn run_approval_action(ctx: &mut Ctx) -> RunApprovalAction {
  ctx.future_action(|request: RunApprovalChanges| async move {
    background::run(move || {
      request
        .changes
        .iter()
        .try_for_each(|(run, change)| apply_in(&request.repo_root, *run, *change))
    })
    .await?
    .map_err(|error| {
      tracing::error!(%error, "cannot change the run's approvals");
      messages::store_error(&error)
    })
  })
}

/// Reads the project's tasks again each time `action` has written its changes; call once, in `create`.
pub(crate) fn reload_after(ctx: &mut Ctx, action: &RunApprovalAction) {
  let Some(nav) = ctx.use_context::<ProjectNav>() else {
    return;
  };
  ctx.watch(&action.state(), move |state| {
    if !state.is_pending() && state.data.is_some() {
      nav.tasks_changed();
    }
  });
}
