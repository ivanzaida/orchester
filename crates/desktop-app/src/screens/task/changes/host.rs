//! A stand-in for the task page's frame until it is built (task P1): it reads the task and its latest run,
//! shows the page tabs with the Changes tab's count, and mounts the Changes tab in the frame's slot. The
//! frame takes this over as the parent module says; this module then goes.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{component::Component, ctx::Ctx, theme::SpacingSize},
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::{ProjectId, TaskId},
  project::TaskKey,
};
use orchester_workspace::{background, messages, runner::state::RunnerState, tasks::detail::TaskDetail};

use super::{ChangesProps, ChangesReads, ChangesTab, Follow, RunChanges, tab_label};
use crate::{
  navigation::Navigation,
  project::{ShownProject, TasksState},
  routes::{ProjectPage, Route, Screen, TaskTab, TaskView},
  screens::placeholder::{centered, titled},
  ui::{
    icon::screen_icon,
    message::message_text,
    state::{load_error, loading},
    tabs::{PageTab, page_tabs},
  },
};

/// A task's page at `view`.
pub fn screen(ctx: &mut Ctx, project: &ShownProject, key: &TaskKey, view: &TaskView) -> Element {
  let task = match project.feed.tasks.get() {
    TasksState::Ready(tasks) => tasks.tasks.iter().find(|task| task.key() == key).map(|task| task.id()),
    TasksState::Loading => return loading("screen-task", &ctx.t("changes.loading")).into(),
    TasksState::Failed(error) => return missing(ctx, &message_text(ctx, &error)),
  };
  let Some(task) = task else {
    return missing(ctx, &ctx.t_args("changes.noTask", [("key", key.to_string())]));
  };
  let online = project
    .feed
    .status
    .get()
    .is_some_and(|status| *status.state() == RunnerState::Online);
  ctx.mount_keyed::<Host>(
    &format!("task-host-{task}"),
    HostProps {
      project: project.open.project.id(),
      repo_root: project.open.repo_root.clone(),
      task,
      changed: (project.feed.changes.get(), project.feed.revision.get()),
      online,
      view: view.clone(),
    },
  )
}

/// Why the task cannot be shown.
fn missing(ctx: &Ctx, why: &str) -> Element {
  let title = ctx.t(Screen::Task.title_key());
  centered(screen_icon(Screen::Task), &title, why)
    .id("screen-task")
    .into()
}

/// The task as read.
#[derive(Clone, PartialEq)]
struct LoadedTask(Arc<TaskDetail>);

impl lurq::app::component::DevtoolsInspectable for LoadedTask {}

#[derive(Clone, PartialEq)]
struct HostProps {
  project: ProjectId,
  repo_root: PathBuf,
  task: TaskId,
  /// How often the project's runs and tasks changed: the task is read again after each.
  changed: (u64, u64),
  online: bool,
  view: TaskView,
}

impl lurq::app::component::DevtoolsInspectable for HostProps {}

struct Host {
  reads: ChangesReads,
  navigation: Navigation,
  /// Counts Try again after the task could not be read.
  reload: Signal<u64>,
}

impl Component for Host {
  type Props = HostProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      reads: ChangesReads::new(ctx),
      navigation: Navigation::of(ctx),
      reload: ctx.signal(0),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<HostProps>().clone();
    let deps = (props.repo_root.clone(), props.task, props.changed, self.reload.get());
    let load = ctx.future(deps, |(repo_root, task, ..)| async move {
      background::run(move || TaskDetail::load_in(&repo_root, task))
        .await?
        .map(|detail| LoadedTask(Arc::new(detail)))
        .map_err(|error| {
          tracing::error!(%error, "cannot read the task");
          messages::store_error(&error)
        })
    });
    let state = load.state().get();
    let Some(LoadedTask(detail)) = state.data else {
      let shown = match state.error {
        // A read again after a failure shows as being read.
        Some(error) if !state.is_pending() => {
          let reload = self.reload.clone();
          let title = ctx.t("changes.taskFailed");
          load_error(ctx, "screen-task", (&title, &message_text(ctx, &error)), move || {
            reload.update(|attempt| *attempt += 1);
          })
        }
        _ => loading("screen-task", &ctx.t("changes.loading")).into(),
      };
      return Column::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::all(SpacingSize::Section))
        .child(shown);
    };
    let tab = props.view.tab();
    let follow = Follow::of(&detail, props.online, tab);
    let changes = self.reads.read(ctx, &props.repo_root, &detail, follow);
    let tabs = self.tabs(ctx, &props, detail.task.key(), changes.as_ref());
    let content = match &props.view {
      TaskView::Changes(file) => ctx.mount_keyed::<ChangesTab>(
        "task-changes-tab",
        ChangesProps {
          project: props.project,
          detail: detail.clone(),
          changes,
          file: file.clone(),
          agent_at_work: follow.agent_at_work,
          reads: self.reads.clone(),
        },
      ),
      TaskView::Summary | TaskView::Conversation => titled(ctx, Screen::Task, &ctx.t(tab.title_key())),
    };
    Column::new()
      .id("screen-task")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .padding(Padding::symmetric(SpacingSize::Section, SpacingSize::Xs).top(SpacingSize::Lg))
          .child(tabs),
      )
      .child(content)
  }
}

impl Host {
  /// The page tabs, Changes with the count of the files it lists once that is known.
  fn tabs(&self, ctx: &mut Ctx, props: &HostProps, key: &TaskKey, changes: Option<&RunChanges>) -> Element {
    let count = changes.and_then(RunChanges::file_count);
    let current = props.view.tab();
    let tabs: Vec<PageTab> = TaskTab::ALL
      .into_iter()
      .map(|tab| {
        let name = ctx.t(tab.title_key());
        let label = match tab {
          TaskTab::Changes => tab_label(ctx, &name, count),
          TaskTab::Summary | TaskTab::Conversation => name,
        };
        let (navigation, project, key) = (self.navigation.clone(), props.project, key.clone());
        PageTab::new(format!("task-tab-{}", tab.name()), label, tab == current, move || {
          navigation.go(Route::Project(
            project,
            ProjectPage::Task(key.clone(), TaskView::of(tab)),
          ));
        })
      })
      .collect();
    page_tabs(ctx, "task-tabs", tabs).into()
  }
}
