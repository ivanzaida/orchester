//! One open project's reading for the notifier: its tasks as they stand with its runner, its runner, its merges, and
//! the Orchestrator's proposals that wait, read again whenever any of them changes. Mounted by the host per open
//! project, beside the project's feed, so it follows the project whether or not one of its tabs is active.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, StreamEmitter},
  },
  node::Element,
};
use orchester_workspace::{
  background,
  messages::{self, Message},
  notifications::{
    WaitingProposal,
    sources::{self, MergeStop, Project, ProjectReading},
    waiting_proposals_in,
  },
  runner::Runners,
  workspace::OpenProject,
};

use super::NotifierLink;
use crate::{
  project::{Projects, TasksState},
  services::Services,
};

#[derive(Clone, PartialEq)]
pub(super) struct ProjectProps {
  pub open: OpenProject,
  pub link: NotifierLink,
}

impl lurq::app::component::DevtoolsInspectable for ProjectProps {}

/// The proposals as read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Proposals(Arc<Vec<WaitingProposal>>);

impl lurq::app::component::DevtoolsInspectable for Proposals {}

/// Reads one project for the notifier. Renders nothing.
pub(super) struct ProjectNotices;

impl Component for ProjectNotices {
  type Props = ProjectProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let ProjectProps { open, link } = ctx.props::<ProjectProps>().clone();
    let services = Services::of(ctx);
    let feed = Projects::of(ctx).feed(open.project.id());
    let root = open.repo_root.clone();
    let tasks = feed.tasks_with_runner(&root);
    let runner = feed.status.get().map(|status| status.state_for(&root));
    let merges = MergeStop::of_jobs(&feed.job_state.get());
    let proposals = proposals(ctx, (services.runners, root));
    // A project is read once its tasks and proposals are: a reading without them would take what waits for gone.
    let (TasksState::Ready(tasks), Some(proposals)) = (tasks, proposals) else {
      return Element::new();
    };
    let project = Project {
      id: open.project.id(),
      name: open.project.name().as_str().into(),
    };
    let reading = ProjectReading {
      tasks: &tasks,
      runner: runner.as_ref(),
      merges: &merges,
      proposals: &proposals.0,
    };
    link.read_project(project.id, sources::of_project(&project, &reading), tasks.needs_you());
    Element::new()
  }
}

/// The proposals that wait in the project at `root`, read again whenever its conversations change; `None` until
/// first read. One that cannot be read is logged and leaves the last reading.
fn proposals(ctx: &mut Ctx, (runners, root): (Runners, PathBuf)) -> Option<Proposals> {
  let chats = ctx.stream(
    (runners, root.clone()),
    |(runners, root), emitter: StreamEmitter<u64, ()>| async move {
      let mut updates = runners.chats_of(&root);
      loop {
        let revision = *updates.borrow_and_update();
        if !emitter.emit(revision) || updates.changed().await.is_err() {
          return;
        }
      }
    },
  );
  let revision = chats.state().get().data.unwrap_or(0);
  let read = ctx
    .future((root, revision), |(root, _)| async move {
      background::run(move || waiting_proposals_in(&root))
        .await?
        .map(|waiting| Proposals(Arc::new(waiting)))
        .map_err(|error| -> Message {
          tracing::error!(%error, "the Orchestrator's proposals cannot be read for notifications");
          messages::store_error(&error)
        })
    })
    .state()
    .get();
  // Proposals that cannot be read hold up nothing else of the project: it is read without them.
  read.data.or_else(|| read.error.is_some().then(Proposals::default))
}
