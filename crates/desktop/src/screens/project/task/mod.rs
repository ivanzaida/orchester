//! A task and its latest run (02.10 `RD4pc`, paused 02.A `Oqmog`, stopped at limit 02.L
//! `D6Nwxz`): the topbar with the way back to the task list and the run controls, the heading,
//! the tabs, and the Summary (`uVIxb`, `F5Lsi`; the plan review 02.11 `x3lVC` while a plan waits for
//! a decision), Conversation (merged with Activity, group `fTfl8`) or Changes tab. The run is read
//! from the project database and read again whenever the runner reports a change or a message is
//! sent; its changes up to its latest checkpoint are read again when it records a new one. Once the
//! run ended, what it left in the repository is read too: Ready for review merges it, and *Remove
//! worktree* clears it. Artifacts come in a later milestone and are shown unavailable.

mod activity;
mod approval_arguments;
mod approval_value;
mod brief;
mod changes;
mod controls;
mod conversation;
mod heading;
mod limit_panel;
mod native_approval;
mod notice;
mod permissions;
mod plan_review;
mod progress;
mod ready;
mod retry_decision;
mod retry_limit;
mod run_again;
mod summary;
mod tab_state;
mod tool_approval;
mod unreadable;
mod worktree;

use std::{sync::Arc, time::SystemTime};

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Text},
  core::Signal,
  layout::{
    Alignment,
    layout_kind::{Justify, ScrollState},
  },
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::TaskId,
  integration::Leftovers,
  limits::ProjectLimits,
  run::{RunStatus, TaskPhase},
  task::TaskStatus,
};
pub(super) use tool_approval::modal as tool_approval_modal;

use self::{
  changes::{ChangesProps, ChangesTab, CheckpointedLoad},
  controls::{ControlAction, RunControl, control_action, control_buttons},
  notice::Lost,
  ready::{ReadyForReview, ReadyProps},
  worktree::{RemoveWorktree, RemoveWorktreeProps},
};
use super::{ProjectNav, page, runner::connection_notice};
use crate::{
  background,
  messages::{self, Message},
  navigation::route::{ChangedFile, Page, TaskPageTab},
  programs::Programs,
  runner::{Runners, state::RunnerState},
  tasks::{
    changes::checkpointed_in,
    data::PipelineChoice,
    detail::TaskDetail,
    integration::{LeftBehind, leftovers_in},
    plan::PlanReview,
  },
  theme::ExtraSpacing,
  ui::{
    field::field_error,
    icon::Icon,
    message_text,
    panel::RecoveryPanel,
    pressable::surface,
    tab::{Tab, tab_row},
  },
  workspace::OpenProject,
};

/// Section tab row (`X7jYe`).
const TAB_ROW_HEIGHT: f32 = 42.0;

/// What a run left, with the tasks revision and merge it was read at.
#[derive(Clone, Debug, PartialEq, Eq)]
struct LeftoversRead {
  read_at: (u64, bool),
  left: LeftBehind,
}

impl lurq::app::component::DevtoolsInspectable for LeftoversRead {}

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
  /// The project's published pipelines, once read: the one a task without a run would use.
  pub pipelines: Option<Arc<Vec<PipelineChoice>>>,
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
  /// Reads the task again once a message was sent.
  reload: Signal<u64>,
  /// Puts the focus in the conversation's composer (Ask agent).
  focus_composer: Signal<bool>,
  /// Why Edit task, opened for this task, was refused and this page shown instead.
  refused_edit: Signal<Option<Message>>,
  nav: ProjectNav,
  runners: Runners,
  scrolls: TaskScrolls,
}

/// Where the task page's scrolled panes are: kept by the page, so a tab that goes inactive and
/// back keeps them.
pub(super) struct TaskScrolls {
  /// The Summary's notice and progress (`ZVdRa`).
  pub(super) progress: ScrollState,
  /// This run's configuration (`F5Lsi`).
  pub(super) this_run: ScrollState,
  /// The Summary of a run that cannot be read (02.R).
  pub(super) unreadable: ScrollState,
}

impl Component for TaskScreen {
  type Props = TaskProps;

  fn create(ctx: &mut Ctx) -> Self {
    control_action(ctx);
    let focus_composer = ctx.signal(false);
    ctx.provide(conversation::ComposerFocus(focus_composer.clone()));
    let nav = ctx
      .use_context::<ProjectNav>()
      .expect("the project shell provides ProjectNav");
    Self {
      requested: ctx.signal(None),
      retry: ctx.signal(0),
      reload: ctx.signal(0),
      focus_composer,
      refused_edit: ctx.signal(None),
      nav,
      runners: ctx
        .use_context::<Runners>()
        .expect("the root provides Runners before mounting screens"),
      scrolls: TaskScrolls {
        progress: ScrollState::new(),
        this_run: ScrollState::new(),
        unreadable: ScrollState::new(),
      },
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TaskProps>().clone();
    if let Some(reason) = self.nav.take_refused_edit(props.task) {
      self.refused_edit.set(Some(reason));
    }
    let control = control_action(ctx);
    let deps = (
      props.project.repo_root.clone(),
      props.task,
      props.revision,
      self.reload.get(),
    );
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
        .child(self.topbar(ctx, &props, (None, Vec::new()), &control))
        .child(centered(panel.into()));
    };
    if self.redirect_queued_retry(&detail) {
      return column;
    }
    if let Some(run) = detail.run.as_ref() {
      ctx.provide(self.nav.integration(run.id()));
    }
    let online = props.runner == RunnerState::Online;
    let lost = detail.lost_its_runner(online).then(|| Lost::of(&props.runner));
    let checkpointed = self.checkpointed(ctx, &props, &detail);
    let files = match &checkpointed {
      Some(CheckpointedLoad::Ready(checkpointed)) => checkpointed.files(),
      _ => None,
    };
    let leftovers = self.leftovers(ctx, &props, &detail);
    let again = run_again::offer(ctx, &props.project, &detail, props.revision, leftovers.as_ref());
    let body = self.body(
      ctx,
      &props,
      &detail,
      (lost, again.reason),
      (checkpointed, leftovers.clone()),
    );
    let runner_notice = connection_notice(ctx, &props.runner, props.runner_since, &self.runners).map(padded);
    let control_error = self.error_rows(ctx, &control);
    let removal = self.removal(ctx, &props, &detail, leftovers.as_ref());
    let ended = removal.into_iter().chain(again.action).collect();
    let earlier_unreadable = detail
      .earlier_unreadable
      .iter()
      .map(|run| unreadable::earlier_notice(ctx, run));
    column
      .child(self.topbar(ctx, &props, (Some(&detail), ended), &control))
      .with_children(runner_notice)
      .child(heading::heading(ctx, &detail, lost.is_some()))
      .with_children(earlier_unreadable)
      .with_children(control_error)
      .child(self.tabs(ctx, &props, files))
      .child(body)
  }
}

impl TaskScreen {
  fn redirect_queued_retry(&self, detail: &TaskDetail) -> bool {
    if detail.task.status() != TaskStatus::Queued || detail.run.is_some() || detail.latest_unreadable.is_some() {
      return false;
    }
    self.nav.replace(Page::RetryTask(detail.task.id()));
    true
  }

  /// The selected tab; the Summary of a run that succeeded is Ready for review (02.13). While the
  /// latest run cannot be read, the Summary is 02.R and no other tab has anything of it to show.
  fn body(
    &self,
    ctx: &mut Ctx,
    props: &TaskProps,
    detail: &Arc<TaskDetail>,
    (lost, reason): (Option<Lost>, Option<Element>),
    (checkpointed, leftovers): (Option<CheckpointedLoad>, Option<Result<Leftovers, Message>>),
  ) -> Element {
    if let Some(run) = &detail.latest_unreadable {
      return match props.tab {
        TaskPageTab::Summary => unreadable::summary(ctx, (run, &self.scrolls.unreadable), &detail.earlier),
        TaskPageTab::Conversation => tab_state::unreadable(ctx, "conversation-unreadable", run),
        TaskPageTab::Changes => tab_state::unreadable(ctx, "changes-unreadable", run),
      };
    }
    let live = detail.phase() == TaskPhase::Running && lost.is_none();
    if props.tab == TaskPageTab::Summary
      && let Some(run) = detail.run.as_ref()
      && let Some(review) = PlanReview::of(run)
    {
      return plan_review::screen(ctx, detail, run, review);
    }
    match (props.tab, detail.run.as_ref()) {
      (TaskPageTab::Summary, Some(run)) if *run.run.status() == RunStatus::Succeeded => ctx
        .mount_keyed::<ReadyForReview>(
          &format!("task-ready-{}", run.id()),
          ReadyProps {
            detail: detail.clone(),
            checkpointed,
            open: props.project.clone(),
            leftovers,
          },
        ),
      (TaskPageTab::Conversation, _) => conversation::conversation_tab(
        ctx,
        detail,
        &conversation::TabContext {
          repo_root: &props.project.repo_root,
          lost,
          leftovers: leftovers.as_ref().and_then(|leftovers| leftovers.as_ref().ok()),
          focus: &self.focus_composer,
          reload: &self.reload,
          revision: props.revision,
        },
      ),
      (TaskPageTab::Summary, _) => summary::summary(
        ctx,
        detail,
        (props.limits, lost, props.pipelines.as_deref().map(Vec::as_slice)),
        (&props.project.repo_root, &self.scrolls),
        reason,
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

  /// What the latest run left in the repository once it ended: its worktrees and branches, read
  /// again whenever the task is; `None` while it has not ended.
  fn leftovers(&self, ctx: &mut Ctx, props: &TaskProps, detail: &TaskDetail) -> Option<Result<Leftovers, Message>> {
    let run = detail.run.as_ref().filter(|run| run.run.status().is_terminal())?;
    let programs = ctx
      .use_context::<Programs>()
      .expect("the root provides Programs before mounting screens");
    let separator = ctx.t("common.listSeparator");
    // A merge or a removal changes what is left: until it is read again, what was read before
    // must not show beside the new merge or reappear after the removal. Runner events for other
    // runs change nothing here, so what was read stays while they are read again.
    let changed_by_person = (props.revision.0, detail.merge.is_some());
    let deps = (
      props.project.repo_root.clone(),
      run.run.clone(),
      props.revision,
      changed_by_person,
    );
    let load = ctx.future(deps, move |(repo_root, run, _, read_at)| {
      let (programs, separator) = (programs.clone(), separator.clone());
      async move {
        background::run(move || leftovers_in(&repo_root, &programs, &run))
          .await?
          .map(|left| LeftoversRead { read_at, left })
          .map_err(|problem| problem.message(&separator))
      }
    });
    let state = load.state().get();
    if state.is_pending() && state.data.as_ref().is_none_or(|read| read.read_at != changed_by_person) {
      return None;
    }
    match (state.data, state.error) {
      (_, Some(error)) => Some(Err(error)),
      (
        Some(LeftoversRead {
          left: LeftBehind(leftovers),
          ..
        }),
        None,
      ) => Some(Ok(leftovers)),
      (None, None) => None,
    }
  }

  /// *Remove worktree* in the topbar, while a run that ended still has worktrees.
  fn removal(
    &self,
    ctx: &mut Ctx,
    props: &TaskProps,
    detail: &Arc<TaskDetail>,
    leftovers: Option<&Result<Leftovers, Message>>,
  ) -> Option<Element> {
    let run = detail.run.as_ref()?;
    let leftovers = leftovers?
      .as_ref()
      .ok()
      .filter(|leftovers| !leftovers.worktrees.is_empty())?;
    Some(ctx.mount_keyed::<RemoveWorktree>(
      &format!("task-remove-worktree-{}", run.id()),
      RemoveWorktreeProps {
        open: props.project.clone(),
        detail: detail.clone(),
        leftovers: leftovers.clone(),
      },
    ))
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
  /// Why the runner did not carry out the last run control, and why Edit task, opened for this
  /// task, was refused (`field-error` below the heading).
  fn error_rows(&self, ctx: &Ctx, control: &ControlAction) -> Vec<Element> {
    let state = control.state().get();
    let control_error = if state.is_pending() { None } else { state.error };
    control_error
      .into_iter()
      .chain(self.refused_edit.get())
      .map(|error| control_error_row(&message_text(ctx, &error)))
      .collect()
  }

  /// The breadcrumb back to the task list, then the run controls (`ctwcG`) and `ended`, the actions
  /// of a run that ended: *Remove worktree* and *Run again*. A run that lost its runner has no
  /// controls: nothing can pause, cancel or continue it until a runner is back, and the runner
  /// notice offers Restart runner. A run that cannot be read shows Cancel run unavailable (`asPAJ`).
  fn topbar(
    &self,
    ctx: &Ctx,
    props: &TaskProps,
    (detail, ended): (Option<&TaskDetail>, Vec<Element>),
    control: &ControlAction,
  ) -> Element {
    let key = detail.map_or_else(String::new, |detail| detail.task.key().to_string());
    let breadcrumb = self.nav.link(Page::TASKS).on(surface().id("task-breadcrumb")).child(
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
    let buttons = match detail {
      Some(detail) if detail.latest_unreadable.is_some() => unreadable::controls(ctx),
      _ => detail
        .filter(|detail| !detail.lost_its_runner(online))
        .and_then(|detail| detail.run.as_ref())
        .map(|run| control_buttons(ctx, run, control, &self.requested))
        .unwrap_or_default(),
    };
    let open = detail.and_then(|detail| retry_limit::open_conversation(ctx, &self.nav, detail));
    let edit = detail
      .filter(|_| open.is_none())
      .and_then(|detail| brief::edit_button(ctx, &self.nav, detail));
    page::topbar_with(
      breadcrumb.into(),
      open.into_iter().chain(buttons).chain(ended).chain(edit),
    )
  }

  /// Summary, Conversation and Changes with the count of files changed up to the latest checkpoint;
  /// Artifacts needs a later milestone (`X7jYe`, `kUKxD`).
  fn tabs(&self, ctx: &Ctx, props: &TaskProps, files: Option<usize>) -> Element {
    let height = Dimension::Pct(100.0);
    let tab = |id, key, value: Option<TaskPageTab>| {
      let item = Tab::new(id, ctx.t(key), height);
      let Some(value) = value else {
        return item;
      };
      item
        .selected(value == props.tab)
        .link(self.nav.link(Page::Task(props.task, value)))
    };
    tab_row(
      Dimension::Px(TAB_ROW_HEIGHT),
      ExtraSpacing::Step.role(),
      Some(SpacingSize::Section),
      [
        tab("task-tab-summary", "runs.page.tabs.summary", Some(TaskPageTab::Summary)),
        tab(
          "task-tab-conversation",
          "runs.page.tabs.conversation",
          Some(TaskPageTab::Conversation),
        ),
        tab("task-tab-changes", "runs.page.tabs.changes", Some(TaskPageTab::Changes)).count(files),
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
