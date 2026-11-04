//! Reads the latest run's pull request and its CI off the UI thread when the page shows the run, then every
//! [`POLL_INTERVAL`] while it stays open (`orchester_workspace::pull_requests::ci::run_ci_in`, which answers from the
//! app's reads of the last 30 s), and again at once on *Check again*. What it read goes into the page's signal, set
//! only when it changed. Renders nothing.

use std::path::PathBuf;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, StreamEmitter},
  },
  components::Column,
  core::Signal,
  node::Element,
};
use orchester_core::{
  id::{ProjectId, RunId},
  pull_request::ci::POLL_INTERVAL,
  time::Timestamp,
};
use orchester_workspace::{
  background,
  pull_requests::ci::{RunCi, run_ci_in},
};

use crate::{project::ShownProject, services::Services};

/// What was read, for run `0`: `None` while its branch has no pull request the last sync stored.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Found(pub Option<(RunId, std::sync::Arc<RunCi>)>);

impl lurq::app::component::DevtoolsInspectable for Found {}

/// One read: the run's pull request with its CI, or none.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Read(Option<std::sync::Arc<RunCi>>);

impl lurq::app::component::DevtoolsInspectable for Read {}

/// The reader of `detail`'s latest run in `project`, putting what it reads into `into`; `again`, *Check again*'s count,
/// and `saves`, the project's, read again. `None` before the task has a run.
pub fn mount(
  ctx: &mut Ctx,
  (project, detail): (&ShownProject, &orchester_workspace::tasks::detail::TaskDetail),
  (again, saves): (u64, u64),
  into: &Signal<Found>,
) -> Option<Element> {
  let run = detail.run.as_ref()?.id();
  let props = ReaderProps {
    root: project.open.repo_root.clone(),
    project: project.open.project.id(),
    run,
    again,
    saves,
    into: into.clone(),
  };
  Some(ctx.mount_keyed::<CiReader>("task-ci-reader", props))
}

#[derive(Clone)]
pub struct ReaderProps {
  pub root: PathBuf,
  pub project: ProjectId,
  pub run: RunId,
  /// *Check again*'s count: a press reads at once.
  pub again: u64,
  /// The project's saves and syncs: a sync can store the run's pull request.
  pub saves: u64,
  pub into: Signal<Found>,
}

/// The page's own signal is the same for its life: it is compared by its id.
impl PartialEq for ReaderProps {
  fn eq(&self, other: &Self) -> bool {
    self.root == other.root
      && self.project == other.project
      && self.run == other.run
      && self.again == other.again
      && self.saves == other.saves
      && self.into.id() == other.into.id()
  }
}

impl lurq::app::component::DevtoolsInspectable for ReaderProps {}

pub struct CiReader;

impl Component for CiReader {
  type Props = ReaderProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ReaderProps>().clone();
    let services = Services::of(ctx);
    let deps = (props.root.clone(), props.project, props.run, props.again, props.saves);
    let state = ctx.stream(
      deps,
      move |(root, project, run, ..), emitter: StreamEmitter<Read, ()>| {
        let services = services.clone();
        async move {
          loop {
            let (reads, programs, root) = (services.ci_reads.clone(), services.programs.clone(), root.clone());
            let read =
              background::run(move || run_ci_in(&reads, (project, &root), &programs, (run, Timestamp::now()))).await;
            match read {
              Ok(Ok(found)) => {
                if !emitter.emit(Read(found.map(std::sync::Arc::new))) {
                  return;
                }
              }
              Ok(Err(error)) => tracing::warn!(%error, %run, "cannot read the run's pull request CI"),
              Err(error) => tracing::warn!(?error, %run, "the read of the run's pull request CI did not finish"),
            }
            tokio::time::sleep(POLL_INTERVAL).await;
          }
        }
      },
    );
    if let Some(Read(found)) = state.state().get().data {
      let found = Found(found.map(|found| (props.run, found)));
      if props.into.get_untracked() != found {
        props.into.set(found);
      }
    }
    Column::new().id("task-ci-reader")
  }
}
