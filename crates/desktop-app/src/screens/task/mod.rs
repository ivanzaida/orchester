//! A task's page on the tab `view` names (`Task · Summary · running · full page · dark` `lUXff`,
//! `Task · Conversation · dark` `kazPZ`, `Task · Changes · dark` `DYun5`). Not built yet but its Conversation
//! tab, which shows through a temporary host ([`ConversationHost`]) until the page's frame (P1) loads the
//! task and mounts the tab itself.

pub mod conversation;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{component::Component, ctx::Ctx},
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::project::TaskKey;
use orchester_workspace::{
  background,
  messages::{self, Message},
  tasks::{data::ProjectTasks, detail::TaskDetail},
};

use super::placeholder::titled;
use crate::{
  project::{ShownProject, TasksState},
  routes::{Screen, TaskView},
  ui::{message::message_text, unavailable::reason_line},
};

pub fn screen(ctx: &mut Ctx, project: &ShownProject, key: &TaskKey, view: &TaskView) -> Element {
  if *view == TaskView::Conversation {
    return ctx.mount_keyed::<ConversationHost>(
      "task-conversation-host",
      HostProps {
        project: project.clone(),
        key: key.clone(),
      },
    );
  }
  let tab = ctx.t(view.tab().title_key());
  let title = ctx.t_args(
    "shell.screens.taskTab",
    [("key", key.to_string()), ("tab", tab.to_string())],
  );
  titled(ctx, Screen::Task, &title)
}

#[derive(Clone, PartialEq)]
struct HostProps {
  project: ShownProject,
  key: TaskKey,
}

impl lurq::app::component::DevtoolsInspectable for HostProps {}

/// Loads the task `key` names, again whenever the project's tasks or runs change, and shows its
/// Conversation tab.
struct ConversationHost;

impl Component for ConversationHost {
  type Props = HostProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let HostProps { project, key } = ctx.props::<HostProps>().clone();
    let tasks = match project.feed.tasks.get() {
      TasksState::Ready(tasks) => Some(tasks),
      TasksState::Loading | TasksState::Failed(_) => None,
    };
    let deps = (
      project.open.repo_root.clone(),
      key.clone(),
      tasks,
      project.feed.changes.get(),
    );
    let load = ctx.future(
      deps,
      |(root, key, tasks, _)| async move { load(root, &key, tasks).await },
    );
    let state = load.state().get();
    let body: Element = match (state.data.flatten(), state.error) {
      (Some(detail), _) => conversation::tab(ctx, &project, &key, detail),
      (None, Some(error)) => reason_line("task-unreadable", &message_text(ctx, &error)).into(),
      (None, None) => Column::new().into(),
    };
    Column::new()
      .id(format!("screen-{}", Screen::Task.name()))
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(body)
  }
}

/// The task `key` of `tasks` with its latest run; `None` while the tasks are not read or none has the key.
async fn load(root: PathBuf, key: &TaskKey, tasks: Option<ProjectTasks>) -> Result<Option<Arc<TaskDetail>>, Message> {
  let Some(task) = tasks.and_then(|tasks| tasks.tasks.iter().find(|task| task.key() == key).map(|task| task.id()))
  else {
    return Ok(None);
  };
  background::run(move || TaskDetail::load_in(&root, task))
    .await?
    .map(|detail| Some(Arc::new(detail)))
    .map_err(|error| {
      tracing::error!(%error, "cannot read the task");
      messages::store_error(&error)
    })
}
