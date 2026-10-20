//! One open project's reading for the notifier: its tasks as they stand with its runner, its runner, its merges, and
//! what its Orchestrator conversations hold (the proposals that wait, and the runs the chat merged itself), read again
//! whenever any of them changes. Mounted by the host per open project, beside the project's feed, so it follows the
//! project whether or not one of its tabs is active.

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
    ChatReading, chats_in,
    sources::{self, MergeStop, Project, ProjectReading},
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

/// The conversations as read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Chats(Arc<ChatReading>);

impl lurq::app::component::DevtoolsInspectable for Chats {}

/// Where reading the conversations stands.
enum ChatsRead {
  /// The first reading is under way.
  Waiting,
  Read(Chats),
  /// The first reading failed; it is tried again when they change.
  Failed,
}

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
    let chats = chats(ctx, (services.runners, root));
    // A project is read once its tasks and conversations are, or its conversations cannot be: a reading before would
    // take a merge the chat carried out for the person's.
    let TasksState::Ready(tasks) = tasks else {
      return Element::new();
    };
    let chats = match chats {
      ChatsRead::Waiting => return Element::new(),
      ChatsRead::Read(chats) => Some(chats),
      ChatsRead::Failed => None,
    };
    let project = Project {
      id: open.project.id(),
      name: open.project.name().as_str().into(),
    };
    let reading = ProjectReading {
      tasks: &tasks,
      runner: runner.as_ref(),
      merges: &merges,
      chat_merges: chats.as_ref().map_or(&[], |chats| chats.0.merged.as_slice()),
    };
    // Proposals that cannot be read are not read as none: once they can, those that waited all along are no news.
    let proposals = chats
      .as_ref()
      .map(|chats| sources::of_proposals(&project, &chats.0.proposals));
    link.read_project(
      project.id,
      (sources::of_project(&project, &reading), proposals),
      tasks.needs_you(),
    );
    Element::new()
  }
}

/// What the conversations of the project at `root` hold, read again whenever they change. One reading that cannot be
/// read is logged and leaves the last one.
fn chats(ctx: &mut Ctx, (runners, root): (Runners, PathBuf)) -> ChatsRead {
  let revisions = ctx.stream(
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
  let revision = revisions.state().get().data.unwrap_or(0);
  let read = ctx
    .future((root, revision), |(root, _)| async move {
      background::run(move || chats_in(&root))
        .await?
        .map(|reading| Chats(Arc::new(reading)))
        .map_err(|error| -> Message {
          tracing::error!(%error, "the Orchestrator's conversations cannot be read for notifications");
          messages::store_error(&error)
        })
    })
    .state()
    .get();
  match (read.data, read.error) {
    (Some(chats), _) => ChatsRead::Read(chats),
    (None, Some(_)) => ChatsRead::Failed,
    (None, None) => ChatsRead::Waiting,
  }
}
