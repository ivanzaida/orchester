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
  task::TaskStatus,
};
use orchester_ipc::WorkSummary;
use orchester_store::ProjectDb;

use super::{
  ProjectNav, TasksState,
  agents::AgentsScreen,
  connections::ConnectionsScreen,
  instructions::InstructionsScreen,
  join::{JoinPage, JoinProps},
  new_task::{Editing, NewTaskForm, NewTaskProps, Recovering},
  overview::{OverviewProps, OverviewScreen},
  pipelines::{PipelinesProps, PipelinesScreen},
  pull_requests::{PullRequestProps, PullRequestScreen, PullRequestsScreen},
  secrets::SecretsScreen,
  servers::ServersScreen,
  settings::SettingsScreen,
  skills::SkillsScreen,
  task::{TaskProps, TaskScreen},
  tasks::{TasksProps, TasksScreen},
  tools::ToolsScreen,
};
use crate::{
  background,
  messages::{self, Message},
  navigation::route::{self, Page, PipelineView, Route},
  runner::{RunnerStatus, state::RunnerState},
  tasks::{
    data::{PipelineChoice, pinned_pipeline},
    usage::ProjectUsage,
  },
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
  /// Why the tasks could not be read again; `tasks` are the ones last read.
  pub stale: Option<Message>,
  pub usage: Option<ProjectUsage>,
  /// How the task list stands with its server; `None` while the project is local.
  pub sharing: Option<crate::sharing::project::state::ProjectSharing>,
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
        agents_working: shown.status.work().map(WorkSummary::agents),
      },
    ),
    Page::Tasks(tab) => ctx.mount_keyed::<TasksScreen>(
      "tasks",
      TasksProps {
        project: open.project.name().as_str().into(),
        root: Arc::new(open.repo_root.clone()),
        runner_online: shown.runner == RunnerState::Online,
        agent_slots: shown.status.work().map(|work| work.agent_slots),
        tab,
        tasks: shown.tasks,
        stale: shown.stale,
        sharing: shown.sharing,
      },
    ),
    Page::ProjectTags => ctx.mount_keyed::<super::tag_manager::TagManager>("project-tags", open),
    Page::NewTask => ctx.mount_keyed::<NewTaskForm>(
      "new-task",
      NewTaskProps {
        project: open,
        pipelines: shown.tasks.ready().map(|tasks| tasks.pipelines.clone()),
        limits,
        editing: None,
        recovering: None,
      },
    ),
    Page::EditTask(task) => edit_task(ctx, open, task, &shown.tasks, limits),
    Page::RetryTask(task) => retry_task(ctx, open, task, &shown.tasks, limits),
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
    Page::Skills(_) => ctx.mount_keyed::<SkillsScreen>(&format!("skills-{}", shown.resources), open),
    Page::Agents(_) => ctx.mount_keyed::<AgentsScreen>(&format!("agents-{}", shown.resources), open),
    Page::Tools(_) => ctx.mount_keyed::<ToolsScreen>(&format!("tools-{}", shown.resources), open),
    Page::Secrets(_) => ctx.mount_keyed::<SecretsScreen>("secrets", open),
    Page::Connections(..) => ctx.mount_keyed::<ConnectionsScreen>("connections", open),
    Page::Settings(_) => ctx.mount_keyed::<SettingsScreen>("settings", open),
    Page::Join(server) => ctx.mount_keyed::<JoinPage>(&format!("join-{server}"), JoinProps { open, server }),
    Page::Servers(..) => ctx.mount_keyed::<ServersScreen>("servers", ()),
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
      recovering: None,
    },
  )
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RecoveredChoice(PipelineChoice);

impl lurq::app::component::DevtoolsInspectable for RecoveredChoice {}

/// Reopen the locked form only for a queued task that has no run, then read its pinned version.
fn retry_task(
  ctx: &mut Ctx,
  project: OpenProject,
  task: TaskId,
  tasks: &TasksState,
  limits: Option<ProjectLimits>,
) -> Element {
  let TasksState::Ready(tasks) = tasks else {
    return edit_task(ctx, project, task, tasks, limits);
  };
  let Some(saved) = tasks.tasks.iter().find(|candidate| candidate.id() == task) else {
    return redirect_to_task(ctx, task);
  };
  if saved.status() != TaskStatus::Queued || tasks.phases.contains_key(&task) {
    return redirect_to_task(ctx, task);
  }
  let Some(pipeline) = saved.details().pipeline else {
    return recovery_error(ctx, "runs.preflight.pipelineNotPublished");
  };
  let Some(version) = saved.pipeline_version() else {
    return recovery_error(ctx, "runs.preflight.pipelineVersionNotPinned");
  };
  let load = ctx.future(
    (project.repo_root.clone(), pipeline, version),
    |(root, id, number)| async move {
      let found = background::run(move || ProjectDb::open(&root).and_then(|db| pinned_pipeline(&db, id, number)))
        .await?
        .map_err(|error| messages::store_error(&error))?;
      found
        .map(RecoveredChoice)
        .ok_or_else(|| Message::new("runs.preflight.pipelineNotPublished"))
    },
  );
  let state = load.state().get();
  let Some(RecoveredChoice(choice)) = state.data else {
    let detail = state
      .error
      .as_ref()
      .map_or_else(|| ctx.t("runs.page.loadingDetail"), |error| message_text(ctx, error));
    let title = ctx.t(if state.error.is_some() {
      "runs.page.loadFailed"
    } else {
      "runs.page.loading"
    });
    let icon = if state.error.is_some() {
      Icon::AlertCircle
    } else {
      Icon::Loader
    };
    return centered(RecoveryPanel::new(icon, &title, &detail).into());
  };
  ctx.mount_keyed::<NewTaskForm>(
    &format!("retry-task-{task}"),
    NewTaskProps {
      project,
      pipelines: Some(tasks.pipelines.clone()),
      limits,
      editing: None,
      recovering: Some(Recovering {
        task: Arc::new(saved.clone()),
        pipeline: Arc::new(choice),
      }),
    },
  )
}

fn redirect_to_task(ctx: &mut Ctx, task: TaskId) -> Element {
  ctx
    .use_context::<ProjectNav>()
    .expect("the project shell provides ProjectNav")
    .replace(Page::task(task));
  Element::new()
}

fn recovery_error(ctx: &Ctx, key: &str) -> Element {
  centered(RecoveryPanel::new(Icon::AlertCircle, &ctx.t("tasks.new.cannotRetry"), &ctx.t(key)).into())
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
