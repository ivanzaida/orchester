//! The screen of each project page, rendered by the router into the shell's outlet with what the
//! shell read about the project.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::Column,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{ResourceId, TaskId},
  limits::ProjectLimits,
};
use orchester_ipc::WorkSummary;

use super::{
  ProjectNav, TasksState,
  agents::AgentsScreen,
  connections::ConnectionsScreen,
  instructions::InstructionsScreen,
  new_task::{Editing, NewTaskForm, NewTaskProps},
  overview::{OverviewProps, OverviewScreen},
  pipelines::{PipelinesProps, PipelinesScreen},
  pull_requests::{PullRequestProps, PullRequestScreen, PullRequestsScreen},
  secrets::SecretsScreen,
  settings::SettingsScreen,
  task::{TaskProps, TaskScreen},
  tasks::{TasksProps, TasksScreen},
  tools::ToolsScreen,
};
use crate::{
  navigation::route::{self, Page, PipelineView, Route},
  runner::{RunnerStatus, state::RunnerState},
  tasks::usage::ProjectUsage,
  ui::{icon::Icon, message_text, panel::RecoveryPanel},
  workspace::OpenProject,
};

/// Provided by the shell on every render, for the page in its outlet.
#[derive(Clone)]
pub struct Shown {
  pub open: OpenProject,
  pub status: RunnerStatus,
  /// The state of this project's runner.
  pub runner: RunnerState,
  /// Changes when tasks were saved or the runner changed a run.
  pub revision: (u64, u64),
  /// Changes when resources were imported; the screens listing them are mounted again.
  pub resources: u64,
  pub tasks: TasksState,
  pub usage: Option<ProjectUsage>,
}

/// The screen of the page the path names. A section (pipelines, instructions, agents, secrets,
/// connections, settings) keeps one screen while its selection changes; the screen reads the
/// selection itself.
pub(crate) fn page(ctx: &mut Ctx) -> Element {
  let shown = ctx
    .use_context::<Shown>()
    .expect("the project shell provides what it read before its outlet");
  // The route guard lets only paths that stand for a page through.
  let Some(page) = Route::current_page(ctx) else {
    return Element::new();
  };
  let open = shown.open.clone();
  let limits = shown.usage.map(|usage| usage.limits);
  let file = route::changed_file(&ctx.route_path());
  match page {
    Page::Overview(tab) => ctx.mount_keyed::<OverviewScreen>(
      "overview",
      OverviewProps {
        open,
        tab,
        tasks: shown.tasks,
        usage: shown.usage,
        runner: shown.runner,
        runner_since: shown.status.trouble_since(),
        agents_working: shown.status.work().map(WorkSummary::agents_working),
      },
    ),
    Page::Tasks(tab) => ctx.mount_keyed::<TasksScreen>(
      "tasks",
      TasksProps {
        project: open.project.name().as_str().into(),
        tab,
        tasks: shown.tasks,
      },
    ),
    Page::NewTask => ctx.mount_keyed::<NewTaskForm>(
      "new-task",
      NewTaskProps {
        project: open,
        pipelines: shown.tasks.ready().map(|tasks| tasks.pipelines.clone()),
        limits,
        editing: None,
      },
    ),
    Page::EditTask(task) => edit_task(ctx, open, task, &shown.tasks, limits),
    Page::Task(task, tab) => ctx.mount_keyed::<TaskScreen>(
      &format!("task-{task}"),
      TaskProps {
        project: open,
        task,
        tab,
        file,
        revision: shown.revision,
        limits,
        pipelines: shown.tasks.ready().map(|tasks| tasks.pipelines.clone()),
        runner: shown.runner,
        runner_since: shown.status.trouble_since(),
      },
    ),
    Page::PullRequests => ctx.mount_keyed::<PullRequestsScreen>("pull-requests", open),
    Page::PullRequest(number) => ctx.mount_keyed::<PullRequestScreen>(
      &format!("pull-request-{number}"),
      PullRequestProps { project: open, number },
    ),
    Page::Pipelines => pipelines(ctx, open, None, shown.resources),
    Page::Pipeline(pipeline, view) => pipelines(ctx, open, Some((pipeline, view)), shown.resources),
    Page::Instructions(..) => ctx.mount_keyed::<InstructionsScreen>(&format!("instructions-{}", shown.resources), open),
    Page::Agents(_) => ctx.mount_keyed::<AgentsScreen>(&format!("agents-{}", shown.resources), open),
    Page::Tools(_) => ctx.mount_keyed::<ToolsScreen>(&format!("tools-{}", shown.resources), open),
    Page::Secrets(_) => ctx.mount_keyed::<SecretsScreen>("secrets", open),
    Page::Connections(..) => ctx.mount_keyed::<ConnectionsScreen>("connections", open),
    Page::Settings(_) => ctx.mount_keyed::<SettingsScreen>("settings", open),
  }
}

/// The New task form on the saved draft `task` (Edit task, `E1o77O`), once the project's tasks are
/// read. A task that is not known leads to its page, which says it cannot be read.
fn edit_task(
  ctx: &mut Ctx,
  project: OpenProject,
  task: TaskId,
  tasks: &TasksState,
  limits: Option<ProjectLimits>,
) -> Element {
  let tasks = match tasks {
    TasksState::Ready(tasks) => tasks,
    TasksState::Loading => {
      let panel = RecoveryPanel::new(
        Icon::Loader,
        &ctx.t("runs.page.loading"),
        &ctx.t("runs.page.loadingDetail"),
      );
      return centered(panel.into());
    }
    TasksState::Failed(error) => {
      let panel = RecoveryPanel::new(
        Icon::AlertCircle,
        &ctx.t("runs.page.loadFailed"),
        &message_text(ctx, error),
      );
      return centered(panel.into());
    }
  };
  let Some(saved) = tasks.tasks.iter().find(|candidate| candidate.id() == task) else {
    ctx
      .use_context::<ProjectNav>()
      .expect("the project shell provides ProjectNav")
      .replace(Page::task(task));
    return Element::new();
  };
  let editing = Editing {
    task: Arc::new(saved.clone()),
    has_run: tasks.phases.contains_key(&task),
  };
  ctx.mount_keyed::<NewTaskForm>(
    &format!("edit-task-{task}"),
    NewTaskProps {
      project,
      pipelines: Some(tasks.pipelines.clone()),
      limits,
      editing: Some(editing),
    },
  )
}

fn centered(content: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .padding(SpacingSize::Section)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(content)
    .into()
}

/// The library, or a pipeline in its editor: one screen, so the library's state survives a
/// visit to the editor, until resources are imported (`resources`).
fn pipelines(
  ctx: &mut Ctx,
  project: OpenProject,
  pipeline: Option<(ResourceId, PipelineView)>,
  resources: u64,
) -> Element {
  ctx.mount_keyed::<PipelinesScreen>(
    &format!("pipelines-{resources}"),
    PipelinesProps {
      project,
      open: pipeline,
    },
  )
}
