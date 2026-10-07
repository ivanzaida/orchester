//! A task and its latest run (02.10 `RD4pc`, paused 02.A `Oqmog`, stopped at limit 02.L
//! `D6Nwxz`): the topbar with the way back to the task list and the run controls, the heading,
//! the tabs, and the Summary (`uVIxb`, `F5Lsi`; the plan review 02.11 `x3lVC` while a plan waits for
//! a decision), Conversation (merged with Activity, group `fTfl8`) or Changes tab. The run is read
//! from the project database and read again whenever the runner reports a change or a message is
//! sent; its changes up to its latest checkpoint are read again when it records a new one, and what
//! its worktree holds beyond that while an agent works, for the Changes tab and its count. Once the
//! run ended or was interrupted, what it left in the repository is read too: Ready for review merges
//! it, and *Remove worktree* clears it; the Summary lists every run of the task (*Runs of this
//! task*, ORC-63) with what each left. Artifacts come in a later milestone and are shown unavailable.

mod activity;
mod approval_arguments;
mod approval_card;
mod approval_value;
pub(in crate::screens::project) mod auto_mode;
mod brief;
mod changes;
mod controls;
mod conversation;
mod dependencies;
mod first_run;
mod heading;
mod history;
mod interrupted;
mod limit_panel;
mod live_time;
mod native_approval;
mod notice;
mod organization;
pub(super) use organization::single_cancel;
mod organization_history;
mod permissions;
mod plan_review;
mod progress;
mod ready;
mod retry_decision;
mod retry_limit;
mod review_note;
mod run_again;
mod run_approvals;
mod scrolls;
mod sharing;
mod summary;
pub(super) use scrolls::TaskScrolls;
mod tab_state;
mod tool_approval;
mod unreadable;
mod unsent_status;
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
pub(in crate::screens) use tool_approval::command_line;
pub(super) use tool_approval::modal as tool_approval_modal;

pub(super) use self::activity::subject_key;
use self::{
  changes::{ChangesProps, ChangesTab, RunChanges},
  controls::{ControlAction, RunControl, control_action, control_buttons},
  notice::Lost,
  ready::{ReadyForReview, ReadyProps},
  worktree::{RemoveWorktree, RemoveWorktreeProps, TaskLeftovers},
};
use super::{ProjectNav, page, runner::connection_notice};
use crate::{
  background,
  messages::{self, Message},
  navigation::route::{ChangedFile, Page, TaskPageTab},
  runner::{Runners, state::RunnerState},
  tasks::{changes::Refresh, data::PipelineChoice, detail::TaskDetail, plan::PlanReview},
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
  /// Wakes the worktree reads while an agent works; they end with the page.
  refresh: Refresh,
  /// Puts the focus in the conversation's composer (Ask agent).
  focus_composer: Signal<bool>,
  /// Why Edit task, opened for this task, was refused and this page shown instead.
  refused_edit: Signal<Option<Message>>,
  nav: ProjectNav,
  runners: Runners,
  scrolls: TaskScrolls,
}

impl Component for TaskScreen {
  type Props = TaskProps;

  fn create(ctx: &mut Ctx) -> Self {
    control_action(ctx);
    first_run::first_run_action(ctx);
    let focus_composer = ctx.signal(false);
    ctx.provide(conversation::ComposerFocus(focus_composer.clone()));
    let nav = ctx
      .use_context::<ProjectNav>()
      .expect("the project shell provides ProjectNav");
    Self {
      requested: ctx.signal(None),
      retry: ctx.signal(0),
      reload: ctx.signal(0),
      refresh: changes::worktree_refresh(ctx),
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
    let first_run = first_run::first_run_action(ctx);
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
      return column
        .child(self.topbar(ctx, &props, (None, (None, None)), &control))
        .child(centered(load_panel(ctx, state.error.as_ref())));
    };
    if self.redirect_queued_retry(&detail) {
      return column;
    }
    if let Some(run) = detail.run.as_ref() {
      ctx.provide(self.nav.integration(run.id()));
    }
    let online = props.runner == RunnerState::Online;
    let lost = detail.lost_its_runner(online).then(|| Lost::of(&props.runner));
    let changes = self.changes(ctx, &props, &detail, lost.is_some());
    let files = changes.as_ref().and_then(RunChanges::listed_files);
    let left = worktree::task_leftovers(ctx, &props, &detail);
    let leftovers = detail.run.as_ref().and_then(|run| left.as_ref()?.of(run.id()));
    let again = run_again::offer(ctx, &props.project, &detail, props.revision, leftovers.as_ref());
    let reason = interrupted::reasons(
      interrupted::continue_unavailable(ctx, &detail)
        .into_iter()
        .chain(again.reason),
    );
    let body = self.body(ctx, &props, &detail, (lost, reason), (changes, leftovers, left.clone()));
    let runner_notice = connection_notice(ctx, &props.runner, props.runner_since, &self.runners).map(padded);
    // A failed reading after a change keeps the task as last read on the page, and says why.
    let control_error = self.error_rows(ctx, (&control, &first_run), state.error);
    let removal = RemoveWorktreeProps::topbar((&props.project, &self.nav), &detail, left.as_ref())
      .map(|removal| ctx.mount_keyed::<RemoveWorktree>(&format!("task-remove-worktree-{}", removal.run.id()), removal));
    // A task without a run has nothing to run again; a queued one may retry its first run.
    let ended = (
      removal,
      again.action.or(first_run::retry_button(ctx, &detail, &first_run)),
    );
    let mut heading = heading::with_notices(ctx, &detail, lost.is_some(), &props.project.repo_root);
    let earlier_unreadable = heading.split_off(1);
    // A task kept Not shared is read-only: its organization shows as plain labels.
    let organization = match sharing::kept_tags(&detail) {
      Some(names) => sharing::kept_organization(ctx, &detail, names),
      None => ctx.mount_keyed::<organization::OrganizationHeader>(
        "task-organization",
        organization::OrganizationProps {
          root: props.project.repo_root.clone(),
          detail: detail.clone(),
        },
      ),
    };
    column
      .child(self.topbar(ctx, &props, (Some(&detail), ended), &control))
      .with_children(runner_notice)
      .with_children(heading)
      .child(organization)
      .with_children(earlier_unreadable)
      .with_children(control_error)
      .child(self.tabs(ctx, &props, files))
      .child(body)
  }
}

impl TaskScreen {
  /// A queued task without a run that this window's New task form queued opens in the locked form
  /// (02.09F); any other, e.g. one automation queued, stays on its page, which offers Retry run.
  fn redirect_queued_retry(&self, detail: &TaskDetail) -> bool {
    if detail.task.status() != TaskStatus::Queued
      || detail.run.is_some()
      || detail.latest_unreadable.is_some()
      || !self.nav.was_queued_by_form(detail.task.id())
    {
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
    (changes, leftovers, left): (
      Option<RunChanges>,
      Option<Result<Leftovers, Message>>,
      Option<TaskLeftovers>,
    ),
  ) -> Element {
    if let Some(run) = &detail.latest_unreadable {
      return match props.tab {
        TaskPageTab::Summary => {
          let runs = history::section(ctx, &props.project, detail, left.as_ref());
          unreadable::summary(ctx, (run, &self.scrolls.unreadable), runs)
        }
        TaskPageTab::Conversation => tab_state::unreadable(ctx, "conversation-unreadable", run),
        TaskPageTab::Changes => tab_state::unreadable(ctx, "changes-unreadable", run),
      };
    }
    if props.tab == TaskPageTab::Summary
      && let Some(run) = detail.run.as_ref()
      && let Some(review) = PlanReview::of(run)
    {
      let runs = history::RunsOfTask {
        open: props.project.clone(),
        detail: detail.clone(),
        left,
      };
      return plan_review::screen(ctx, runs, run, review);
    }
    match (props.tab, detail.run.as_ref()) {
      (TaskPageTab::Summary, Some(run)) if *run.run.status() == RunStatus::Succeeded => ctx
        .mount_keyed::<ReadyForReview>(
          &format!("task-ready-{}", run.id()),
          ReadyProps {
            detail: detail.clone(),
            checkpointed: changes.map(|changes| changes.checkpointed),
            open: props.project.clone(),
            leftovers,
            left,
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
      (TaskPageTab::Summary, _) => {
        let runs = history::section(ctx, &props.project, detail, left.as_ref());
        summary::summary(
          ctx,
          detail,
          (props.limits, lost, props.pipelines.as_deref().map(Vec::as_slice)),
          (&props.project.repo_root, &self.scrolls, props.revision),
          (reason, runs),
        )
      }
      (TaskPageTab::Changes, _) => ctx.mount_keyed::<ChangesTab>(
        "task-changes",
        ChangesProps {
          repo_root: props.project.repo_root.clone(),
          detail: detail.clone(),
          changes: changes.unwrap_or_default(),
          file: props.file.clone(),
          retry: self.retry.clone(),
        },
      ),
    }
  }

  /// The latest run's changes, read once for the Changes tab and the count on its label; `None`
  /// without a run.
  fn changes(&self, ctx: &mut Ctx, props: &TaskProps, detail: &TaskDetail, lost: bool) -> Option<RunChanges> {
    let run = detail.run.as_ref()?;
    let repo_root = &props.project.repo_root;
    let at_work = (agent_at_work(detail, lost), props.tab == TaskPageTab::Changes);
    Some(RunChanges {
      checkpointed: changes::checkpointed(ctx, repo_root, run, self.retry.get()),
      worktree: changes::not_checkpointed(ctx, repo_root, run, &self.refresh, at_work),
    })
  }
}

impl TaskScreen {
  /// Why the runner did not carry out the last run control or Retry run, why Edit task, opened for
  /// this task, was refused, and why the task could not be read again (`reread`) (`field-error`
  /// below the heading).
  fn error_rows(
    &self,
    ctx: &Ctx,
    (control, first_run): (&ControlAction, &first_run::FirstRunAction),
    reread: Option<Message>,
  ) -> Vec<Element> {
    let state = control.state().get();
    let control_error = if state.is_pending() { None } else { state.error };
    let first_run = first_run.state().get();
    let first_run_error = if first_run.is_pending() { None } else { first_run.error };
    control_error
      .into_iter()
      .chain(first_run_error)
      .chain(self.refused_edit.get())
      .chain(reread)
      .map(|error| control_error_row(&message_text(ctx, &error)))
      .collect()
  }

  /// The breadcrumb back to the task list, then *Remove worktree* of a run with no live work, the
  /// run controls (`ctwcG`) and *Run again* (63.04 `rkAU3`, 63.05 `LmTnc`). A run that lost its runner has no
  /// controls: nothing can pause, cancel or continue it until a runner is back, and the runner
  /// notice offers Restart runner. A run that cannot be read shows Cancel run unavailable (`asPAJ`).
  fn topbar(
    &self,
    ctx: &mut Ctx,
    props: &TaskProps,
    (detail, (removal, again)): (Option<&TaskDetail>, (Option<Element>, Option<Element>)),
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
    if let Some(detail) = detail.filter(|detail| sharing::kept_tags(detail).is_some()) {
      let share = ctx.mount_keyed::<sharing::ShareTask>(
        "task-share",
        sharing::ShareTaskProps {
          root: props.project.repo_root.clone(),
          task: detail.task.id(),
          key: detail.task.key().to_string(),
        },
      );
      return page::topbar_with(breadcrumb.into(), [share]);
    }
    let online = props.runner == RunnerState::Online;
    let buttons = match detail {
      Some(detail) if detail.latest_unreadable.is_some() => unreadable::controls(ctx),
      _ => detail
        .filter(|detail| !detail.lost_its_runner(online))
        .and_then(|detail| detail.run.as_ref().map(|run| (run, !detail.blocked_by.is_empty())))
        .map(|(run, blocked)| control_buttons(ctx, run, (control, blocked), &self.requested))
        .unwrap_or_default(),
    };
    let open = detail.and_then(|detail| retry_limit::open_conversation(ctx, &self.nav, detail));
    let edit = detail
      .filter(|_| open.is_none())
      .and_then(|detail| brief::edit_button(ctx, &self.nav, detail));
    // On the Conversation tab it would open the tab itself.
    let open = open.filter(|_| props.tab != TaskPageTab::Conversation);
    page::topbar_with(
      breadcrumb.into(),
      open.into_iter().chain(removal).chain(buttons).chain(again).chain(edit),
    )
  }

  /// Summary, Conversation and Changes with the count of the files it lists; Artifacts needs a later
  /// milestone (`X7jYe`, `kUKxD`).
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

/// An agent is at work on the run: its worktree changes and its activity grows.
fn agent_at_work(detail: &TaskDetail, lost: bool) -> bool {
  detail.phase() == TaskPhase::Running && !lost
}

/// Why the runner did not carry out a run control (`field-error` below the heading).
fn control_error_row(text: &str) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_horizontal(SpacingSize::Section)
    .padding_top(SpacingSize::Md)
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

fn load_panel(ctx: &Ctx, error: Option<&Message>) -> Element {
  let panel = match error {
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
  panel.into()
}
