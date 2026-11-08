//! What the CI parts of a task's page start: *Fix CI* (`orchester_workspace::pull_requests::ci_fix::fix_ci`, on the
//! app's runtime, so a send under way ends even once the page closes), the run's *Fix CI automatically* and its limit,
//! *Check again*, and opening a pull request's or a check's page on GitHub. The task page makes them once and provides
//! them to its header, Summary and Conversation; a Fix CI sent, or a switch set, reads the task again.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::ctx::{Ctx, FutureAction},
  core::Signal,
};
use orchester_core::{
  id::{ProjectId, RunId, TaskId},
  pull_request::ci::fix::AutoFix,
  run::Answerer,
};
use orchester_workspace::{
  background,
  messages::{self, Message},
  pull_requests::{
    ci::ReadCi,
    ci_fix::{Fixer, fix_ci, set_auto_fix_in},
  },
  workspace::OpenProject,
};

use crate::{project::ProjectFeed, services::Services};

/// A press of *Fix CI* for the checks `read` of `task`'s latest run `run`, by `by`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixRequest {
  pub project: OpenProject,
  pub task: TaskId,
  pub run: RunId,
  pub read: ReadCi,
  /// The run is recorded as running while no runner looks after it.
  pub lost: bool,
  pub by: Answerer,
}

impl lurq::app::component::DevtoolsInspectable for FixRequest {}

/// *Fix CI automatically* of `run` set to `auto` by `by`, which the workspace refuses unless a person.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutoRequest {
  pub root: PathBuf,
  pub run: RunId,
  pub auto: AutoFix,
  pub by: Answerer,
}

impl lurq::app::component::DevtoolsInspectable for AutoRequest {}

/// The page's CI actions.
#[derive(Clone)]
pub struct CiActions {
  pub fix: FutureAction<FixRequest, (), Message>,
  pub auto: FutureAction<AutoRequest, (), Message>,
  /// *Check again*'s count, which the reader reads again at.
  pub again: Signal<u64>,
}

impl CiActions {
  /// The actions of the page of `feed`'s project: made once, in the page's `create`.
  pub fn new(ctx: &mut Ctx, feed: &ProjectFeed) -> Self {
    let actions = Self {
      fix: fix_action(ctx),
      auto: auto_action(ctx),
      again: ctx.signal(0),
    };
    for state in [actions.fix.state(), actions.auto.state()] {
      let feed = feed.clone();
      ctx.watch(&state, move |result| {
        if result.is_fulfilled() {
          feed.saved();
        }
      });
    }
    actions
  }

  pub fn of(ctx: &mut Ctx) -> Self {
    ctx
      .use_context::<Self>()
      .expect("the task page provides its CI actions before mounting its parts")
  }

  /// *Check again*: the next read of pull request `number` of `project` asks GitHub, and it reads now.
  pub fn check_again(
    &self,
    services: &Services,
    (project, number): (ProjectId, u64),
  ) -> impl Fn() + Send + Sync + use<> {
    let (reads, again) = (services.ci_reads.clone(), self.again.clone());
    move || {
      reads.forget(project, number);
      again.update(|count| *count += 1);
    }
  }
}

/// Who pressed now: automation driving the window, or a person.
pub fn pressed_by() -> Answerer {
  if lurq::app::events::is_synthetic_input() {
    Answerer::Automation
  } else {
    Answerer::Person
  }
}

/// Opens `url` in the browser.
pub fn open_page(services: &Services, url: Arc<str>) -> impl Fn() + Send + Sync + 'static {
  let links = services.links.clone();
  move || {
    if let Err(error) = links.open(&url) {
      tracing::warn!(%error, "cannot open the GitHub page in the browser");
    }
  }
}

fn fix_action(ctx: &mut Ctx) -> FutureAction<FixRequest, (), Message> {
  let services = Services::of(ctx);
  ctx.future_action(move |request: FixRequest| {
    let services = services.clone();
    let runtime = services.runtime.clone();
    async move {
      let sending = async move {
        let runner = (&services.runners, &services.servers);
        let fixer = Fixer::Hand { by: request.by };
        let work = (request.task, request.run);
        let ready = (&services.workspace, &services.vault);
        fix_ci(
          runner,
          ready,
          &request.project,
          work,
          (request.read, request.lost),
          fixer,
        )
        .await
        .map(|_| ())
      };
      runtime.spawn(sending).await.map_err(|error| {
        tracing::error!(%error, "Fix CI did not finish");
        messages::interrupted()
      })?
    }
  })
}

fn auto_action(ctx: &mut Ctx) -> FutureAction<AutoRequest, (), Message> {
  ctx.future_action(move |request: AutoRequest| async move {
    background::run(move || set_auto_fix_in(&request.root, request.run, request.auto, request.by))
      .await?
      .map_err(|error| {
        tracing::warn!(?error, "Fix CI automatically was not set");
        error.message()
      })
  })
}
