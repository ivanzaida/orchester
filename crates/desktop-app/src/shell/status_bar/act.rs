//! What the bar's items do in the window: open the shown project's runner log or start its runner again
//! (the runners show the active tab's project), open a place where a press asks, leave a failure in the
//! activity's history, and stop runs auto-approving, written off the UI thread, after which every tab of
//! the project reads its tasks again.

use std::{path::PathBuf, sync::Arc};

use lurq::app::ctx::{Ctx, FutureAction};
use orchester_core::id::RunId;
use orchester_workspace::{
  background,
  messages::{self, Message},
  tasks::run_approvals::stop_auto_approving_in,
};
use parking_lot::Mutex;

use super::facts::{BarAction, Dispatch, Stopping};
use crate::{navigation::Navigation, project::ProjectFeed, services::Services, shell::update::Restart};

/// Stops runs of a project auto-approving.
#[derive(Clone, PartialEq)]
pub struct StopRequest {
  asked: StopKey,
  /// The project's feed, which reads its tasks again once the runs stopped.
  feed: ProjectFeed,
}

impl lurq::app::component::DevtoolsInspectable for StopRequest {}

/// Which runs of which project a stop is about.
#[derive(Clone, Debug, PartialEq, Eq)]
struct StopKey {
  repo_root: PathBuf,
  runs: Vec<RunId>,
}

pub type StopAction = FutureAction<StopRequest, ProjectFeed, Message>;

/// Stopping runs auto-approving in this window: which runs it was last asked to stop, so its progress
/// and its failure show only beside those runs of that project.
#[derive(Default)]
pub struct Stopper {
  asked: Arc<Mutex<Option<StopKey>>>,
  /// Whether the action's end is followed already; once per bar.
  followed: Mutex<bool>,
}

impl Stopper {
  /// The action. lurq keeps a component's futures by the order a render asks for them, so the bar asks
  /// for it first in every render, before its streams.
  pub fn action(&self, ctx: &mut Ctx) -> StopAction {
    let action = ctx.future_action(|request: StopRequest| async move {
      let StopRequest { asked, feed } = request;
      // Every run auto-approving when it writes stops, not only the runs the bar showed.
      background::run(move || stop_auto_approving_in(&asked.repo_root).map(|_stopped| ()))
        .await?
        .map_err(|error| {
          // The runs still auto-approve, so the bar still says so and *Stop* can be pressed again.
          tracing::error!(%error, "cannot stop runs auto-approving");
          messages::store_error(&error)
        })?;
      Ok(feed)
    });
    let mut followed = self.followed.lock();
    if !*followed {
      // A failed run keeps the data of the run before it: only a fulfilled one stopped the runs.
      ctx.watch(&action.state(), |state| {
        if let (true, Some(feed)) = (state.is_fulfilled(), &state.data) {
          feed.saved();
        }
      });
      *followed = true;
    }
    action
  }

  /// How stopping `runs` of the project at `repo_root` goes: what `action` did only when it was last asked
  /// to stop exactly those, so another project's stop, or a reason left from runs that changed, does not
  /// show.
  pub fn stopping(&self, action: &StopAction, repo_root: &std::path::Path, runs: &[RunId]) -> Stopping {
    let asked = self.asked.lock();
    let about = asked
      .as_ref()
      .is_some_and(|asked| asked.repo_root == repo_root && asked.runs == runs);
    if !about {
      return Stopping::Idle;
    }
    let state = action.state().get();
    match (state.is_pending(), state.is_rejected(), state.error) {
      (true, ..) => Stopping::Busy,
      (false, true, Some(reason)) => Stopping::Failed(reason),
      _ => Stopping::Idle,
    }
  }
}

/// Where the bar's actions go in this window; `shown` is the shown project's folder and feed.
pub fn dispatch(
  ctx: &mut Ctx,
  services: &Services,
  shown: Option<(PathBuf, ProjectFeed)>,
  (stop, stopper): (&StopAction, &Stopper),
) -> Dispatch {
  let navigation = Navigation::of(ctx);
  let restart = Restart::of(ctx);
  let (services, stop, asked) = (services.clone(), stop.clone(), stopper.asked.clone());
  Arc::new(move |action| match action {
    BarAction::OpenRunnerLog => {
      if let Some((repo_root, _)) = &shown {
        services.runners.open_log_of(repo_root);
      }
    }
    BarAction::RestartRunner => services.runners.restart(),
    BarAction::Go(route, at) => navigation.open_item(route, at),
    BarAction::Resolve(id) => services.activity.resolve(id),
    BarAction::ReviewInterrupted(project) => services.recovery.review(project, true),
    BarAction::RestartToUpdate => restart.press(),
    BarAction::OpenFile(path) => {
      if let Err(error) = orchester_exec::external::open_path(&path) {
        tracing::warn!(%error, file = %path.display(), "cannot open the file");
      }
    }
    BarAction::ReadAgain => {
      if let Some((_, feed)) = &shown {
        feed.saved();
      }
    }
    BarAction::StopAutoApproving(runs) => {
      // The bar offers *Stop* only for the shown project's runs; one stop at a time.
      let Some((repo_root, feed)) = &shown else {
        return;
      };
      if stop.state().get_untracked().is_pending() {
        return;
      }
      let key = StopKey {
        repo_root: repo_root.clone(),
        runs,
      };
      *asked.lock() = Some(key.clone());
      stop.run(StopRequest {
        asked: key,
        feed: feed.clone(),
      });
    }
  })
}
