//! The screen of each project page, rendered by the router into the shell's outlet with what the
//! shell read about the project.

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::id::ResourceId;
use orchester_ipc::WorkSummary;

use super::{
  TasksState,
  agents::AgentsScreen,
  connections::ConnectionsScreen,
  new_task::{NewTaskForm, NewTaskProps},
  overview::{OverviewProps, OverviewScreen},
  pipelines::{PipelinesProps, PipelinesScreen},
  pull_requests::{PullRequestProps, PullRequestScreen, PullRequestsScreen},
  settings::SettingsScreen,
  task::{TaskProps, TaskScreen},
  tasks::{TasksProps, TasksScreen},
};
use crate::{
  navigation::route::{self, Page, Route},
  runner::{RunnerStatus, state::RunnerState},
  tasks::usage::ProjectUsage,
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
  pub tasks: TasksState,
  pub usage: Option<ProjectUsage>,
}

/// The screen of the page the path names. A section (pipelines, agents, connections, settings)
/// keeps one screen while its selection changes; the screen reads the selection itself.
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
      },
    ),
    Page::Task(task, tab) => ctx.mount_keyed::<TaskScreen>(
      &format!("task-{task}"),
      TaskProps {
        project: open,
        task,
        tab,
        file,
        revision: shown.revision,
        limits,
        runner: shown.runner,
        runner_since: shown.status.trouble_since(),
      },
    ),
    Page::PullRequests => ctx.mount_keyed::<PullRequestsScreen>("pull-requests", open),
    Page::PullRequest(number) => ctx.mount_keyed::<PullRequestScreen>(
      &format!("pull-request-{number}"),
      PullRequestProps { project: open, number },
    ),
    Page::Pipelines => pipelines(ctx, open, None),
    Page::Pipeline(pipeline) => pipelines(ctx, open, Some(pipeline)),
    Page::Agents(_) => ctx.mount_keyed::<AgentsScreen>("agents", open),
    Page::Connections(..) => ctx.mount_keyed::<ConnectionsScreen>("connections", open),
    Page::Settings(_) => ctx.mount_keyed::<SettingsScreen>("settings", open),
  }
}

/// The library, or `pipeline` in its editor: one screen, so the library's state survives a
/// visit to the editor.
fn pipelines(ctx: &mut Ctx, project: OpenProject, pipeline: Option<ResourceId>) -> Element {
  ctx.mount_keyed::<PipelinesScreen>(
    "pipelines",
    PipelinesProps {
      project,
      open: pipeline,
    },
  )
}
