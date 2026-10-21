//! The project's Overview (layer `Overview` `vgqpM`, `Overview · full page · dark` `q9mf0k`): what needs
//! the person, the running work, the queued and failed tasks, the recent runs and today's usage, from the
//! project feed. Above its heading, the notice of the runs the previous session left interrupted (`JSRBO`)
//! and the runner's when it could not restart (`KGpMB`); under it, the tasks other tasks block and those
//! whose latest run cannot be read (`Bq82T`). Its other states: a project without tasks (`a5CcQx`), the
//! tasks being read (`CHfHU`), interrupted runs (`JAIYi`) and the *Needs you* tab (`mEvfK`).
//!
//! What counts and how is `orchester_workspace::tasks::{board, attention, overview}`; the tasks are the
//! feed's as they stand with the runner, so a run that lost its agent is interrupted. The line over the
//! title names the repository and branch as git tells them (`orchester_workspace::tasks::checkout`, read
//! off the UI thread). How long each thing waits and *Completed today* are taken again every [`TICK`].
//! Not built here: Import resources (visibly unavailable), and answering a request or a question in place:
//! each card opens its task where it is answered, and the project's memory records open its Memory page.

mod activity;
mod header;
mod metrics;
mod needs;
mod notices;
mod ready;
mod row;
mod tasks;

use std::time::Duration;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Interval},
    theme::SpacingSize,
  },
  components::{Column, Row, ScrollVertical},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ProjectId, project::TaskKey, time::Timestamp};
use orchester_workspace::tasks::{attention, data::ProjectTasks};

use crate::{
  lifecycle::recovery::RecoveryNotice,
  navigation::{Navigation, link::Open},
  project::{ShownProject, TasksState},
  routes::{OverviewTab, ProjectPage, Route, TaskTab, TaskView},
  theme::OverviewSpacing,
  ui::state::load_error,
};

/// The *Needs you* column's width (`Needs you` `NbKDR`).
const NEEDS_WIDTH: f32 = 360.0;
/// How often the waits and *Completed today* move on while the Overview shows, as the Needs you panel's
/// waits do; *today* rolls over at local midnight within one tick.
const TICK: Duration = Duration::from_secs(30);

pub fn screen(ctx: &mut Ctx, project: &ShownProject, tab: OverviewTab) -> Element {
  let key = format!("overview-{}", project.open.project.id());
  ctx.mount_keyed::<OverviewScreen>(
    &key,
    OverviewProps {
      project: project.clone(),
      tab,
    },
  )
}

#[derive(Clone, PartialEq)]
struct OverviewProps {
  project: ShownProject,
  tab: OverviewTab,
}

impl DevtoolsInspectable for OverviewProps {}

struct OverviewScreen {
  tick: Signal<u64>,
  interval: Interval,
}

impl Component for OverviewScreen {
  type Props = OverviewProps;

  fn create(ctx: &mut Ctx) -> Self {
    let tick = ctx.signal(0_u64);
    let next = tick.clone();
    let interval = ctx.create_interval(TICK, move || next.update(|tick| *tick += 1));
    Self { tick, interval }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // Read on every tick, so the times below are taken again.
    self.tick.get();
    self.interval.start();
    let OverviewProps { project, tab } = ctx.props::<OverviewProps>().clone();
    let recovery = ctx.mount_keyed::<RecoveryNotice>("recovery-notice", project.clone());
    let state = project.tasks();
    let mut body = Column::new()
      .id("overview-body")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Xl)
      .with_children(notices::runner(ctx, &project))
      .child(header::header(ctx, &project));
    body = match &state {
      TasksState::Failed(error) => body.child(failed(ctx, &project, error)),
      TasksState::Loading => body
        .child(metrics::metrics(ctx, &project, None))
        .child(main(ctx, &project, tab, None)),
      TasksState::Ready(tasks) => body
        .with_children(notices::tasks(ctx, project.open.project.id(), tasks))
        .child(metrics::metrics(ctx, &project, Some(tasks)))
        .child(main(ctx, &project, tab, Some(tasks))),
    };
    let page = Column::new()
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(SpacingSize::Section)
          .bottom(SpacingSize::Section)
          .left(OverviewSpacing::Page.role())
          .right(OverviewSpacing::Page.role()),
      )
      .child(recovery)
      .child(body);
    Column::new()
      .id("screen-overview")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(
        ScrollVertical::new(page)
          .id("overview-scroll")
          .width(Dimension::Pct(100.0))
          .flex(1.0),
      )
  }
}

/// The tasks, or the empty project, and the recent runs, with what needs the person beside them; `None`
/// while the tasks are read.
fn main(ctx: &mut Ctx, project: &ShownProject, tab: OverviewTab, tasks: Option<&ProjectTasks>) -> Row {
  let id = project.open.project.id();
  let name = project.open.project.name().as_str();
  let work = match tasks {
    Some(tasks) if tasks.tasks.is_empty() => header::empty_project(ctx, project),
    tasks => tasks::section(ctx, (id, name), tab, tasks).into(),
  };
  let now = Timestamp::now();
  let left = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Xl)
    .child(work)
    .child(activity::section(ctx, id, tasks));
  let items = tasks.map(attention::items);
  Row::new()
    .id("overview-main")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Section)
    .align_items(Alignment::Start)
    .child(left)
    .child(
      needs::column(ctx, id, items.as_deref(), now)
        .width(Dimension::Px(NEEDS_WIDTH))
        .flex_shrink(0.0),
    )
}

/// The tasks could not be read: why, and Try again, which reads them again.
fn failed(ctx: &Ctx, project: &ShownProject, error: &orchester_workspace::messages::Message) -> Element {
  let reason = crate::ui::message::message_text(ctx, error);
  let body = ctx.t_args("overview.loadError.body", [("reason", reason)]);
  let feed = project.feed.clone();
  load_error(
    ctx,
    "overview-load-error",
    (&ctx.t("overview.loadError.title"), &body),
    move || feed.saved(),
  )
}

/// What opens `task` of `project` on `tab` where a press asks (`navigation::link`).
fn open_task(ctx: &mut Ctx, project: ProjectId, task: &str, tab: TaskTab) -> Open {
  let task: TaskKey = task.parse().expect("a task's key reads back as a task key");
  Navigation::of(ctx).to(Route::Project(project, ProjectPage::Task(task, TaskView::of(tab))))
}
