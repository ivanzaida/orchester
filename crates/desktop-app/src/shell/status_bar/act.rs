//! What the bar's items do in the window: open the shown project's runner log or start its runner again
//! (the runners show the active tab's project), go somewhere in the active tab, leave a failure in the
//! activity's history, and stop runs auto-approving, written off the UI thread, after which every tab of
//! the project reads its tasks again.

use std::{path::PathBuf, sync::Arc};

use lurq::app::ctx::{Ctx, FutureAction};
use orchester_core::id::RunId;
use orchester_workspace::{
  background,
  messages::{self, Message},
  tasks::run_approvals::{RunApprovalChange, apply_in},
};

use super::facts::{BarAction, Dispatch};
use crate::{navigation::Navigation, project::ProjectFeed, services::Services};

/// Stops runs of a project auto-approving.
#[derive(Clone, PartialEq)]
pub struct StopRequest {
  repo_root: PathBuf,
  runs: Vec<RunId>,
  /// The project's feed, which reads its tasks again once the runs stopped.
  feed: ProjectFeed,
}

impl lurq::app::component::DevtoolsInspectable for StopRequest {}

pub type StopAction = FutureAction<StopRequest, ProjectFeed, Message>;

/// The action that stops runs auto-approving; create it once, in the bar's `create`.
pub fn stop_action(ctx: &mut Ctx) -> StopAction {
  let action = ctx.future_action(|request: StopRequest| async move {
    let StopRequest { repo_root, runs, feed } = request;
    background::run(move || {
      runs
        .iter()
        .try_for_each(|run| apply_in(&repo_root, *run, RunApprovalChange::AutoApprove(false)))
    })
    .await?
    .map_err(|error| {
      // The runs still auto-approve, so the bar still says so and *Stop* can be pressed again.
      tracing::error!(%error, "cannot stop runs auto-approving");
      messages::store_error(&error)
    })?;
    Ok(feed)
  });
  ctx.watch(&action.state(), |state| {
    if let (false, Some(feed)) = (state.is_pending(), &state.data) {
      feed.saved();
    }
  });
  action
}

/// Where the bar's actions go in this window; `shown` is the shown project's folder and feed.
pub fn dispatch(
  ctx: &mut Ctx,
  services: &Services,
  shown: Option<(PathBuf, ProjectFeed)>,
  stop: &StopAction,
) -> Dispatch {
  let navigation = Navigation::of(ctx);
  let (services, stop) = (services.clone(), stop.clone());
  Arc::new(move |action| match action {
    BarAction::OpenRunnerLog => services.runners.open_log(),
    BarAction::RestartRunner => services.runners.restart(),
    BarAction::Go(route) => navigation.go(route),
    BarAction::Resolve(id) => services.activity.resolve(id),
    BarAction::StopAutoApproving(runs) => {
      // The bar offers *Stop* only for the shown project's runs; one stop at a time.
      let Some((repo_root, feed)) = &shown else {
        return;
      };
      if !stop.state().get_untracked().is_pending() {
        stop.run(StopRequest {
          repo_root: repo_root.clone(),
          runs,
          feed: feed.clone(),
        });
      }
    }
  })
}
