//! A task and its latest run (02.10 `RD4pc`, paused 02.A `Oqmog`, stopped at limit 02.L
//! `D6Nwxz`): the topbar with the way back to the task list and the run controls, the heading,
//! the tabs, and the Summary (`uVIxb`, `F5Lsi`), Changes or Activity (02.I `SfXU1`) tab. The run is
//! read from the project database and read again whenever the runner reports a change; its
//! changes up to its latest checkpoint are read again when it records a new one. Conversation and
//! Artifacts come in later milestones and are shown unavailable.

mod activity;
mod changes;
mod controls;
mod heading;
mod limit_panel;
mod notice;
mod permissions;
mod progress;
mod ready;
mod retry_limit;
mod summary;
mod tab_state;

use std::{sync::Arc, time::SystemTime};

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::TaskId,
  limits::ProjectLimits,
  run::{RunStatus, TaskPhase},
};

use self::{
  activity::{ActivityProps, ActivityTab},
  changes::{ChangesProps, ChangesTab, CheckpointedLoad},
  controls::{ControlAction, RunControl, control_action, control_buttons},
  notice::Lost,
};
use super::{ProjectNav, page, runner::connection_notice};
use crate::{
  background, messages,
  navigation::route::{ChangedFile, Page, TaskPageTab},
  programs::Programs,
  runner::{Runners, state::RunnerState},
  tasks::{changes::checkpointed_in, detail::TaskDetail},
  theme::ExtraSpacing,
  ui::{
    field::field_error,
    icon::Icon,
    message_text,
    panel::RecoveryPanel,
    pressable::{pressable, surface},
    tab::{Tab, tab_row},
  },
  workspace::OpenProject,
};

/// Section tab row (`X7jYe`).
const TAB_ROW_HEIGHT: f32 = 42.0;

#[derive(Clone, Debug, PartialEq)]
pub struct TaskProps {
  pub project: OpenProject,
  pub task: TaskId,
  pub tab: TaskPageTab,
  /// The changed file the path names, for the Changes tab.
  pub file: Option<ChangedFile>,
  /// Changes when tasks were saved or the runner changed a run: the task is read again.
  pub revision: (u64, u64),
  /// The project's limits, once read: the most a raised limit may be.
  pub limits: Option<ProjectLimits>,
  /// The project's runner, and since when it has had trouble: a run recorded as running is
  /// interrupted while it is not online, and 02.H (`EFY7I`) shows while it could not restart.
  pub runner: RunnerState,
  pub runner_since: Option<SystemTime>,
}

impl lurq::app::component::DevtoolsInspectable for TaskProps {}

/// The task as read, shared with the tabs.
#[derive(Clone, Debug, PartialEq)]
struct LoadedTask(Arc<TaskDetail>);

impl lurq::app::component::DevtoolsInspectable for LoadedTask {}

pub struct TaskScreen {
  /// The last run control requested; under way while the control action is pending.
  requested: Signal<Option<RunControl>>,
  /// Reads the run's checkpointed changes again after they could not be read.
  retry: Signal<u64>,
  nav: ProjectNav,
  runners: Runners,
}

impl Component for TaskScreen {
  type Props = TaskProps;

  fn create(ctx: &mut Ctx) -> Self {
    control_action(ctx);
    Self {
      requested: ctx.signal(None),
      retry: ctx.signal(0),
      nav: ctx
        .use_context::<ProjectNav>()
        .expect("the project shell provides ProjectNav"),
      runners: ctx
        .use_context::<Runners>()
        .expect("the root provides Runners before mounting screens"),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TaskProps>().clone();
    let control = control_action(ctx);
    let deps = (props.project.repo_root.clone(), props.task, props.revision);
    let load = ctx.future(deps, |(repo_root, task, _)| async move {
      background::run(move || TaskDetail::load_in(&repo_root, task))
        .await?
        .map(|detail| LoadedTask(Arc::new(detail)))
        .map_err(|error| {
          tracing::error!(%error, "cannot read the task");
          messages::store_error(&error)
        })
    });
    let state = load.state().get();
    let column = Column::new()
      .id("task-page")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0));
    let Some(LoadedTask(detail)) = state.data else {
      let panel = match &state.error {
        Some(error) => RecoveryPanel::new(
          Icon::AlertCircle,
          &ctx.t("runs.page.loadFailed"),
          &message_text(ctx, error),
        ),
        None => RecoveryPanel::new(
          Icon::Loader,
          &ctx.t("runs.page.loading"),
          &ctx.t("runs.page.loadingDetail"),
        ),
      };
      return column
        .child(self.topbar(ctx, &props, None, &control))
        .child(centered(panel.into()));
    };
    let online = props.runner == RunnerState::Online;
    let lost = detail.lost_its_runner(online).then(|| Lost::of(&props.runner));
    let checkpointed = self.checkpointed(ctx, &props, &detail);
    let files = match &checkpointed {
      Some(CheckpointedLoad::Ready(checkpointed)) => checkpointed.files(),
      _ => None,
    };
    let body = self.body(ctx, &props, &detail, lost, checkpointed);
    let runner_notice = connection_notice(ctx, &props.runner, props.runner_since, &self.runners).map(padded);
    let control_error = control
      .state()
      .get()
      .error
      .filter(|_| !control.state().get().is_pending())
      .map(|error| control_error_row(&message_text(ctx, &error)));
    column
      .child(self.topbar(ctx, &props, Some(&detail), &control))
      .with_children(runner_notice)
      .child(heading::heading(ctx, &detail, lost.is_some()))
      .with_children(control_error)
      .child(self.tabs(ctx, &props, files))
      .child(body)
  }
}

impl TaskScreen {
  /// The selected tab; the Summary of a run that succeeded is Ready for review (02.13).
  fn body(
    &self,
    ctx: &mut Ctx,
    props: &TaskProps,
    detail: &Arc<TaskDetail>,
    lost: Option<Lost>,
    checkpointed: Option<CheckpointedLoad>,
  ) -> Element {
    let live = detail.phase() == TaskPhase::Running && lost.is_none();
    match (props.tab, detail.run.as_ref()) {
      (TaskPageTab::Summary, Some(run)) if *run.run.status() == RunStatus::Succeeded => {
        ready::ready_for_review(ctx, run, checkpointed.as_ref(), props.project.project.name().as_str())
      }
      (TaskPageTab::Summary, _) => summary::summary(ctx, detail, props.limits, lost, &props.project.repo_root),
      (TaskPageTab::Activity, _) => ctx.mount_keyed::<ActivityTab>(
        "task-activity",
        ActivityProps {
          repo_root: props.project.repo_root.clone(),
          detail: detail.clone(),
          revision: props.revision,
          live,
        },
      ),
      (TaskPageTab::Changes, _) => ctx.mount_keyed::<ChangesTab>(
        "task-changes",
        ChangesProps {
          repo_root: props.project.repo_root.clone(),
          detail: detail.clone(),
          checkpointed: checkpointed.unwrap_or(CheckpointedLoad::Loading),
          file: props.file.clone(),
          live,
          retry: self.retry.clone(),
        },
      ),
    }
  }

  /// The latest run's changes from its base to its latest checkpoint, read again when it records
  /// a new one; `None` without a run.
  fn checkpointed(&self, ctx: &mut Ctx, props: &TaskProps, detail: &TaskDetail) -> Option<CheckpointedLoad> {
    let run = detail.run.as_ref()?;
    let programs = ctx
      .use_context::<Programs>()
      .expect("the root provides Programs before mounting screens");
    let head = run
      .run
      .last_checkpoint()
      .map(|checkpoint| checkpoint.revision().clone());
    let deps = (
      props.project.repo_root.clone(),
      run.run.base().clone(),
      head,
      self.retry.get(),
    );
    let load = ctx.future(deps, move |(repo_root, base, head, _)| {
      let programs = programs.clone();
      async move {
        background::run(move || checkpointed_in(&repo_root, &programs, &base, head.as_ref()))
          .await?
          .map_err(|error| {
            tracing::warn!(%error, "cannot read the run's changes");
            error.message()
          })
      }
    });
    let state = load.state().get();
    Some(match (state.data, state.error) {
      (_, Some(error)) => CheckpointedLoad::Failed(error),
      (Some(checkpointed), None) => CheckpointedLoad::Ready(checkpointed),
      (None, None) => CheckpointedLoad::Loading,
    })
  }
}

impl TaskScreen {
  /// The breadcrumb back to the task list, then the run controls (`ctwcG`). A run that lost
  /// its runner has none: nothing can pause, cancel or continue it until a runner is back, and
  /// the runner notice offers Restart runner.
  fn topbar(&self, ctx: &Ctx, props: &TaskProps, detail: Option<&TaskDetail>, control: &ControlAction) -> Element {
    let nav = self.nav.clone();
    let key = detail.map_or_else(String::new, |detail| detail.task.key().to_string());
    let breadcrumb = pressable(surface().id("task-breadcrumb"), move || nav.show(Page::TASKS)).child(
      Text::new(&ctx.t_args(
        "runs.page.breadcrumb",
        [
          ("project", props.project.project.name().as_str()),
          ("task", key.as_str()),
        ],
      ))
      .variant(TypographyStyle::Description)
      .nowrap(),
    );
    let online = props.runner == RunnerState::Online;
    let buttons = detail
      .filter(|detail| !detail.lost_its_runner(online))
      .and_then(|detail| detail.run.as_ref())
      .map(|run| control_buttons(ctx, run, control, &self.requested))
      .unwrap_or_default();
    page::topbar_with(breadcrumb.into(), buttons)
  }

  /// Summary, Changes with the count of files changed up to the latest checkpoint, and Activity;
  /// the others need later milestones (`X7jYe`, `kUKxD`).
  fn tabs(&self, ctx: &Ctx, props: &TaskProps, files: Option<usize>) -> Element {
    let height = Dimension::Pct(100.0);
    let tab = |id, key, value: Option<TaskPageTab>| {
      let item = Tab::new(id, ctx.t(key), height);
      let Some(value) = value else {
        return item;
      };
      let (nav, task) = (self.nav.clone(), props.task);
      item
        .selected(value == props.tab)
        .on_click(move || nav.show(Page::Task(task, value)))
    };
    tab_row(
      Dimension::Px(TAB_ROW_HEIGHT),
      ExtraSpacing::Step.role(),
      Some(SpacingSize::Section),
      [
        tab("task-tab-summary", "runs.page.tabs.summary", Some(TaskPageTab::Summary)),
        tab("task-tab-conversation", "runs.page.tabs.conversation", None),
        tab("task-tab-changes", "runs.page.tabs.changes", Some(TaskPageTab::Changes)).count(files),
        tab(
          "task-tab-activity",
          "runs.page.tabs.activity",
          Some(TaskPageTab::Activity),
        ),
        tab("task-tab-artifacts", "runs.page.tabs.artifacts", None),
      ],
    )
  }
}

/// Why the runner did not carry out a run control (`field-error` below the heading).
fn control_error_row(text: &str) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_horizontal(SpacingSize::Section)
    .padding_bottom(SpacingSize::Md)
    .child(field_error(text))
    .into()
}

/// The runner notice (02.H `EFY7I`) above the heading, 32 below the topbar as on the overview
/// (`c7qiCh`).
fn padded(notice: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_horizontal(SpacingSize::Section)
    .padding_top(SpacingSize::Section)
    .child(notice)
    .into()
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
