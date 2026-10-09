//! The task page's header (`Task header` `x8Dmfa` in `lUXff`): the breadcrumb, the title with Edit,
//! *Remove worktree*, the run controls and Run again at its right, the status and run line, the frame's
//! notices, Run again's confirmation in place, and the page tabs (`Page tabs` `L9OIIZ`), Artifacts and
//! Memory locked (`Smb4t`). It is the same component on every tab, so a request under way or an open
//! confirmation stays as the tabs switch.

mod controls;
mod notices;
mod run_again;
mod run_line;

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::project::TaskKey;
use orchester_workspace::{
  messages::Message,
  tasks::{detail::TaskDetail, integration_jobs::ProjectJobState},
};

use self::{
  controls::{ControlAction, Controls, RunControl, control_action, controls},
  run_again::{Confirmation, Confirming, StartAction},
};
use super::{
  PageState, RemovalPlace,
  read::TaskLeftovers,
  summary::runs::{Runs, removal::confirm_for},
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, Route, TaskList, TaskTab, TaskView},
  services::Services,
  theme::ExtraSpacing,
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    button::{ActionButton, ButtonKind},
    icon::Icon,
    tabs::{PageTab, page_tabs},
    tooltip::Truncated,
  },
};

#[derive(Clone, PartialEq)]
pub struct HeaderProps {
  pub project: ShownProject,
  pub detail: Arc<TaskDetail>,
  pub tab: TaskTab,
  /// The project's runner is online: a run recorded as running without one is interrupted.
  pub online: bool,
  pub left: TaskLeftovers,
  pub jobs: ProjectJobState,
  /// Why the task could not be read again; the page shows it as last read.
  pub stale: Option<Message>,
}

impl lurq::app::component::DevtoolsInspectable for HeaderProps {}

pub struct TaskHeader {
  /// The control last requested; under way while the control action is pending.
  requested: Signal<Option<RunControl>>,
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
      confirming: ctx.signal(Confirming::default()),
      remove: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<HeaderProps>().clone();
    let services = Services::of(ctx);
    let (open, feed) = (props.project.open.clone(), props.project.feed.clone());
    let detail = props.detail.clone();
    let runners = (services.runners.clone(), services.servers.clone());
    let action = control_action(ctx, (open.clone(), detail.task.id()), runners.clone());
    let saves = feed.revision.get() + feed.outside.get().tasks;
    let check = run_again::check(ctx, (open.repo_root.clone(), services.programs), &detail, saves);
    let start = run_again::start_action(ctx, runners, (open.repo_root, feed), self.confirming.clone());
    let (title_row, refused) = self.title_row(ctx, &props, (&action, check.as_ref()));
    let lost = detail.lost_its_runner(props.online);
    let confirmation = check
      .as_ref()
      .and_then(|check| check.as_ref().ok())
      .and_then(|check| self.confirmation(ctx, &props, check, &start));
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
      .child(run_line::status_row(ctx, &detail, lost))
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
  /// The title and its actions with, under them, why the ones that cannot be used now are
  /// unavailable; and the control the runner last refused, with why.
  fn title_row(
    &self,
    ctx: &mut Ctx,
    props: &HeaderProps,
    (action, check): (
      &ControlAction,
      Option<&Result<orchester_workspace::tasks::run_again::RunAgainCheck, Message>>,
    ),
  ) -> (Element, Option<(RunControl, Message)>) {
    let detail = &props.detail;
    let state = action.state().get();
    let requested = self.requested.get();
    let pending = if state.is_pending() { requested } else { None };
    let refused = match (state.is_pending(), requested, state.error) {
      (false, Some(control), Some(error)) => Some((control, error)),
      _ => None,
    };
    let (buttons, mut reasons) = controls(
      ctx,
      &Controls {
        detail,
        online: props.online,
        action,
        pending,
        requested: &self.requested,
      },
    );
    let latest_busy = detail.run.as_ref().is_some_and(|run| props.jobs.run(run.id()).busy());
    let again = run_again::button(ctx, check, (&self.confirming, &self.remove), latest_busy);
    if let Some(reason) = again.as_ref().and_then(|(_, reason)| reason.clone()) {
      reasons.push(reason);
    }
    let actions: Vec<Element> = edit_button(ctx, props)
      .into_iter()
      .chain(remove_button(ctx, props))
      .chain(buttons)
      .chain(again.map(|(button, _)| button))
      .map(Element::from)
      .collect();
    let title = Truncated::new(
      "task-title",
      detail.task.details().title.as_str().to_owned(),
      TypographyStyle::Heading,
    )
    .render(ctx);
    let row = Row::new()
      .id("task-title-row")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Start)
      .child(Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(title))
      .child(
        Column::new()
          .spacing(ExtraSpacing::Snug.role())
          .align_items(Alignment::End)
          .child(
            Row::new()
              .id("task-actions")
              .spacing(SpacingSize::Sm)
              .align_items(Alignment::Center)
              .with_children(actions),
          )
          .with_children(reasons.iter().enumerate().map(|(index, reason)| {
            Text::new(reason)
              .id(format!("task-action-reason-{index}"))
              .variant(TypographyStyle::Caption)
              .describe("role", "unavailable-reason")
          })),
      );
    (row.into(), refused)
  }

  /// Run again's confirmation, while it is open and the task can run again.
  fn confirmation(
    &self,
    ctx: &mut Ctx,
    props: &HeaderProps,
    check: &orchester_workspace::tasks::run_again::RunAgainCheck,
    start: &StartAction,
  ) -> Option<Element> {
    let Confirming(Some(key)) = self.confirming.get() else {
      return None;
    };
    let (run, next) = (props.detail.run.as_ref()?, check.next.as_ref().ok()?);
    let leftovers = props.left.of(run.id()).and_then(Result::ok);
    Some(run_again::confirmation(
      ctx,
      &Confirmation {
        detail: &props.detail,
        run,
        check,
        next,
        key,
        leftovers: leftovers.as_ref(),
        remove: &self.remove,
        open: &self.confirming,
        action: start,
      },
    ))
  }
}

fn task_route(props: &HeaderProps, tab: TaskTab) -> Route {
  let key: TaskKey = props.detail.task.key().clone();
  Route::Project(
    props.project.open.project.id(),
    ProjectPage::Task(key, TaskView::of(tab)),
  )
}

fn crumbs(ctx: &mut Ctx, props: &HeaderProps) -> Element {
  let navigation = Navigation::of(ctx);
  let project = props.project.open.project.id();
  let (overview, tasks) = (navigation.clone(), navigation);
  let name = props.project.open.project.name().as_str().to_owned();
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
      Crumb::here(props.detail.task.key().to_string()).mono(),
    ],
  )
  .into()
}

/// Edit, while the task can be edited.
fn edit_button(ctx: &mut Ctx, props: &HeaderProps) -> Option<ActionButton> {
  if !props.detail.editable() {
    return None;
  }
  let navigation = Navigation::of(ctx);
  let route = Route::Project(
    props.project.open.project.id(),
    ProjectPage::EditTask(props.detail.task.key().clone()),
  );
  Some(
    ActionButton::new("task-edit", ButtonKind::Ghost, ctx.t("task.edit"))
      .icon(Icon::Pencil)
      .on_click(move || navigation.go(route.clone())),
  )
}

/// *Remove worktree* of the latest run, once it has no live work and still has worktrees: it opens the
/// confirmation in the run's row on the Summary.
fn remove_button(ctx: &mut Ctx, props: &HeaderProps) -> Option<ActionButton> {
  let run = props.detail.run.as_ref()?;
  let record = props.detail.history.iter().find(|record| record.id == run.id())?;
  record.without_live_work()?;
  let left = props.left.of(run.id())?.ok()?;
  if left.worktrees.is_empty() {
    return None;
  }
  let button =
    ActionButton::new("task-remove-worktree", ButtonKind::Ghost, ctx.t("task.removeWorktree")).icon(Icon::FolderX);
  if props.jobs.run(run.id()).removing() {
    return Some(button.loading());
  }
  if props
    .detail
    .history
    .iter()
    .any(|record| props.jobs.run(record.id).busy())
  {
    return Some(button.disabled());
  }
  let state = PageState::of(ctx);
  // Its confirmation is open already.
  if state.removal.get().run == Some(run.id()) {
    return Some(button.disabled());
  }
  let job = props.project.feed.jobs.of(run.id());
  let id = run.id();
  Some(button.on_click(move || {
    job.clear_failed_removal();
    state.open_removal(id, RemovalPlace::Header);
  }))
}

/// The latest run's *Remove worktree* confirmation, while the header's button opened it.
fn removal(ctx: &mut Ctx, props: &HeaderProps) -> Option<Element> {
  let run = props.detail.run.as_ref()?.id();
  let removal = PageState::of(ctx).removal.get();
  if !removal.shows(run, RemovalPlace::Header) {
    return None;
  }
  let runs = Runs {
    project: &props.project,
    detail: &props.detail,
    left: &props.left,
    jobs: &props.jobs,
    removal,
  };
  confirm_for(ctx, &runs, run)
}

/// Summary, Conversation and Changes, then Artifacts and Memory, which are not built yet.
fn tabs(ctx: &mut Ctx, props: &HeaderProps) -> Element {
  let navigation = Navigation::of(ctx);
  let open = |tab: TaskTab| {
    let (navigation, route) = (navigation.clone(), task_route(props, tab));
    move || navigation.go(route.clone())
  };
  let tab = |ctx: &Ctx, tab: TaskTab| {
    PageTab::new(
      format!("task-tab-{}", tab.name()),
      ctx.t(tab.title_key()),
      props.tab == tab,
      open(tab),
    )
  };
  // The Changes tab's count of changed files (`Changes 4`) comes with the Changes tab itself (D1).
  let tabs = [
    tab(ctx, TaskTab::Summary),
    tab(ctx, TaskTab::Conversation),
    tab(ctx, TaskTab::Changes),
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
