//! The task page's header (`Task header` `x8Dmfa` in `lUXff`): the breadcrumb, the title with its
//! actions (`actions`), the status and run line, the frame's notices, Run again's and the latest run's
//! *Remove worktree* confirmations in place, and the page tabs (`Page tabs` `L9OIIZ`), Artifacts and
//! Memory locked (`Smb4t`). It is the same component on every tab, so a request under way or an open
//! confirmation stays as the tabs switch.

mod actions;
mod controls;
mod notices;
mod refusal;
mod run_again;
mod run_line;

use std::sync::Arc;

use lurq::{
  app::{component::Component, ctx::Ctx, theme::SpacingSize},
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::project::TaskKey;
use orchester_workspace::{messages::Message, tasks::run_again::RunAgainCheck};

use self::{
  controls::{Controls, Requested, control_action, controls},
  refusal::Refusals,
  run_again::{Confirmation, Confirming, StartAction},
};
use super::{
  PageState, RemovalPlace, TaskContext,
  summary::runs::{Runs, removal::confirm_for},
};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskList, TaskTab, TaskView},
  services::Services,
  theme::ExtraSpacing,
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    tabs::{PageTab, page_tabs},
  },
};

#[derive(Clone, PartialEq)]
pub struct HeaderProps {
  pub task: TaskContext,
  pub tab: TaskTab,
  /// Why the task could not be read again; the page shows it as last read.
  pub stale: Option<Message>,
}

impl lurq::app::component::DevtoolsInspectable for HeaderProps {}

pub struct TaskHeader {
  /// The control last pressed; under way while the control action is pending.
  requested: Signal<Option<Requested>>,
  refusals: Refusals,
  /// Run again's confirmation.
  confirming: Signal<Confirming>,
  /// Its switch that removes the worktrees of the run before, off each time it opens.
  remove: Signal<bool>,
}

impl Component for TaskHeader {
  type Props = HeaderProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      requested: ctx.signal(None),
      refusals: Refusals::new(ctx),
      confirming: ctx.signal(Confirming::default()),
      remove: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<HeaderProps>().clone();
    let services = Services::of(ctx);
    let (open, feed) = (props.task.project.open.clone(), props.task.project.feed.clone());
    let detail = props.task.detail.clone();
    let runners = (services.runners.clone(), services.servers.clone());
    let action = control_action(ctx, (open.clone(), detail.task.id()), runners.clone());
    let again = (
      feed.revision.get() + feed.outside.get().tasks,
      feed.changes.get(),
      feed.outside.get().limits,
    );
    let check = run_again::check(ctx, (open.repo_root.clone(), services.programs), &detail, again);
    let start = run_again::start_action(ctx, runners, (open, feed), self.confirming.clone());
    let state = action.state().get();
    let requested = self.requested.get();
    let pending = state
      .is_pending()
      .then_some(requested)
      .flatten()
      .map(|requested| requested.control);
    let refused = self.refusals.shown(&state, requested, &detail);
    let mut shown = controls(
      ctx,
      &Controls {
        detail: &detail,
        online: props.task.online,
        action: &action,
        pending,
        requested: &self.requested,
        approval_waits: props.task.waiting_approval().is_some(),
      },
    );
    let latest_busy = detail
      .run
      .as_ref()
      .is_some_and(|run| props.task.jobs.run(run.id()).busy());
    shown.extend(run_again::button(
      ctx,
      check.as_ref(),
      (&self.confirming, &self.remove),
      (latest_busy, props.task.online),
    ));
    let title_row = actions::title_row(ctx, &props, shown);
    let confirmation = check
      .as_ref()
      .and_then(|check| check.as_ref().ok())
      .and_then(|check| self.confirmation(ctx, &props, check, (&start, latest_busy)));
    Column::new()
      .id("task-header")
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(SpacingSize::Lg)
          .left(SpacingSize::Section)
          .right(SpacingSize::Section),
      )
      .spacing(SpacingSize::Sm)
      .child(crumbs(ctx, &props))
      .child(title_row)
      .child(run_line::status_row(ctx, &props.task))
      .with_children(notices::notices(ctx, &detail, props.stale.as_ref(), refused))
      .with_children(confirmation)
      .with_children(removal(ctx, &props))
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .padding(Padding::new().top(ExtraSpacing::Loose.role()))
          .child(tabs(ctx, &props)),
      )
  }
}

impl TaskHeader {
  /// Run again's confirmation, while it is open and the task can run again. `busy`: git removes or
  /// merges the latest run's worktrees.
  fn confirmation(
    &self,
    ctx: &mut Ctx,
    props: &HeaderProps,
    check: &RunAgainCheck,
    (start, busy): (&StartAction, bool),
  ) -> Option<Element> {
    let Confirming(Some(key)) = self.confirming.get() else {
      return None;
    };
    let (run, next) = (props.task.detail.run.as_ref()?, check.next.as_ref().ok()?);
    let leftovers = props.task.left.of(run.id()).and_then(Result::ok);
    Some(run_again::confirmation(
      ctx,
      &Confirmation {
        detail: &props.task.detail,
        run,
        check,
        next,
        key,
        leftovers: leftovers.as_ref(),
        remove: &self.remove,
        open: &self.confirming,
        action: start,
        busy,
      },
    ))
  }
}

fn task_route(props: &HeaderProps, tab: TaskTab) -> Route {
  let key: TaskKey = props.task.detail.task.key().clone();
  Route::Project(
    props.task.project.open.project.id(),
    ProjectPage::Task(key, TaskView::of(tab)),
  )
}

fn crumbs(ctx: &mut Ctx, props: &HeaderProps) -> Element {
  let navigation = Navigation::of(ctx);
  let project = props.task.project.open.project.id();
  let (overview, tasks) = (navigation.clone(), navigation);
  let name = props.task.project.open.project.name().as_str().to_owned();
  breadcrumb(
    ctx,
    "task-crumbs",
    CrumbSeparator::Slash,
    vec![
      Crumb::link(name, move || {
        overview.go(Route::Project(project, ProjectPage::Overview(Default::default())));
      }),
      Crumb::link(ctx.t("task.crumbs.tasks"), move || {
        tasks.go(Route::Project(project, ProjectPage::Tasks(TaskList::default())));
      }),
      Crumb::here(props.task.detail.task.key().to_string()).mono(),
    ],
  )
  .into()
}

/// The latest run's *Remove worktree* confirmation, while the header's button opened it.
fn removal(ctx: &mut Ctx, props: &HeaderProps) -> Option<Element> {
  let run = props.task.detail.run.as_ref()?.id();
  let removal = PageState::of(ctx).removal.get();
  if !removal.shows(run, RemovalPlace::Header) {
    return None;
  }
  let runs = Runs {
    task: &props.task,
    removal,
  };
  confirm_for(ctx, &runs, run)
}

/// Summary, Conversation and Changes with the count of the files it lists, then Artifacts and Memory,
/// which are not built yet.
fn tabs(ctx: &mut Ctx, props: &HeaderProps) -> Element {
  let navigation = Navigation::of(ctx);
  let open = |tab: TaskTab| {
    let (navigation, route) = (navigation.clone(), task_route(props, tab));
    move || navigation.go(route.clone())
  };
  let tab = |tab: TaskTab, label: Arc<str>| {
    PageTab::new(format!("task-tab-{}", tab.name()), label, props.tab == tab, open(tab))
  };
  let changes = ctx.t(TaskTab::Changes.title_key());
  let count = super::changes_count(ctx, &props.task);
  let changes = super::changes::tab_label(ctx, &changes, count);
  let tabs = [
    tab(TaskTab::Summary, ctx.t(TaskTab::Summary.title_key())),
    tab(TaskTab::Conversation, ctx.t(TaskTab::Conversation.title_key())),
    tab(TaskTab::Changes, changes),
    PageTab::unavailable(
      "task-tab-artifacts",
      ctx.t("task.tabs.artifacts"),
      ctx.t("task.tabs.artifactsReason"),
    ),
    PageTab::unavailable(
      "task-tab-memory",
      ctx.t("task.tabs.memory"),
      ctx.t("task.tabs.memoryReason"),
    ),
  ];
  page_tabs(ctx, "task-tabs", tabs).into()
}
