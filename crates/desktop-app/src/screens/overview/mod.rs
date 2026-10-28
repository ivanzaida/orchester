//! The project's Overview (`Overview · redesign` `iRvT6`): one calm page that says what waits for the person, what is
//! moving and what landed. Under the header (`header`), in the main column: *Waiting for you* (`digest`), *At work*
//! (`at_work`) and *Queue* (`queue`), or, when nothing waits and nothing runs, the quiet card with *Ready to start*
//! (`quiet`, 02 `HWznK`); a project without tasks shows its getting-started list instead (`start`, 03 `ArvFR`). In the
//! side column: *Plan limits* (`limits`), *Landed* (`landed`) and *Delivery* (`delivery`). While the tasks are read
//! the sections' rows are bones (`skeleton`, 04 `A3qRTI`); what is wrong outside the page stands under the header
//! (`notices`, 05 `JzNRO`). Above everything, the notice of the runs the previous session left interrupted.
//!
//! Where the data comes from: the project feed's tasks (`ProjectFeed::tasks_with_runner`), read again only when a run
//! changes state or the tasks are saved; with them the feed reads who works on each running run and when each merged
//! run merged (`orchester_workspace::tasks::at_work`), so the page never reads a run of its own. The runner's slots
//! are its status report, the plan limits the session's readings, the team server the status bar's stream. Only
//! Delivery and the getting-started list read the project's database themselves, off the UI thread and only when its
//! tasks or setup were saved. Ages move on every [`TICK`].

mod at_work;
mod delivery;
mod digest;
mod header;
mod landed;
mod limits;
mod notices;
mod queue;
mod quiet;
mod section;
mod skeleton;
mod start;

use std::{num::NonZeroU32, time::Duration};

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
use orchester_core::{connection::plan_limits::PlanReadings, time::Timestamp};
use orchester_workspace::{
  overview::{
    at_work::at_work as runs_at_work,
    landed::{landed as merged, queue_line},
    limits::near_limits,
    queue::queue as queued_tasks,
  },
  tasks::{attention, data::ProjectTasks},
};

use self::skeleton::{BonesProps, bones};
use crate::{
  lifecycle::recovery::RecoveryNotice,
  project::{ShownProject, TasksState},
  services::{
    accounts::{self, Accounts},
    plan_limits,
  },
  theme::{ExtraSpacing, OverviewSpacing},
  ui::failure::Failure,
};

/// The side column's width (`Side` `NLdTo`).
const SIDE_WIDTH: f32 = 344.0;
/// How often the ages move on while the Overview shows; *today* rolls over at local midnight within one tick.
const TICK: Duration = Duration::from_secs(30);
/// The bones' titles in each section while the tasks are read (04 `A3qRTI`).
const WAIT_BONES: &[f32] = &[200.0, 260.0, 180.0, 230.0];
const RUN_BONES: &[f32] = &[200.0, 180.0, 210.0];
const SIDE_BONES: &[f32] = &[170.0, 140.0, 190.0];

pub fn screen(ctx: &mut Ctx, project: &ShownProject) -> Element {
  let key = format!("overview-{}", project.open.project.id());
  ctx.mount_keyed::<OverviewScreen>(
    &key,
    OverviewProps {
      project: project.clone(),
    },
  )
}

#[derive(Clone, PartialEq)]
struct OverviewProps {
  project: ShownProject,
}

impl DevtoolsInspectable for OverviewProps {}

struct OverviewScreen {
  tick: Signal<u64>,
  interval: Interval,
}

/// What the page knows besides the tasks: the accounts and their readings, the runner's slots, and the time.
struct Around {
  accounts: Accounts,
  readings: PlanReadings,
  slots: Option<(usize, NonZeroU32)>,
  now: Timestamp,
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
    // Read on every tick, so the ages below are taken again.
    self.tick.get();
    self.interval.start();
    let OverviewProps { project } = ctx.props::<OverviewProps>().clone();
    let recovery = ctx.mount_keyed::<RecoveryNotice>("recovery-notice", project.clone());
    let around = Around {
      accounts: accounts::listed(ctx),
      readings: plan_limits::account_readings(ctx),
      slots: project
        .feed
        .status
        .get()
        .as_ref()
        .and_then(|status| status.work())
        .map(|work| work.agents()),
      now: Timestamp::now(),
    };
    let state = project.tasks();
    let page = Column::new()
      .id("overview-body")
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(SpacingSize::Xl)
          .bottom(SpacingSize::Xl)
          .left(ExtraSpacing::Roomy.role())
          .right(ExtraSpacing::Roomy.role()),
      )
      .spacing(ExtraSpacing::Tabs.role())
      .child(recovery)
      .child(header::header(ctx, &project))
      .with_children(notices::runner(ctx, &project))
      .with_children(notices::server(
        ctx,
        &project,
        state.ready().is_some_and(|tasks| tasks.shared),
      ))
      .child(match &state {
        TasksState::Failed(error) => {
          let feed = project.feed.clone();
          Failure::new("overview-load-error", ctx.t("overview.loadError.title"), error.clone())
            .still_works(ctx.t("overview.loadError.stillWorks"))
            .log(project.open.repo_root.clone())
            .retry(false, move || feed.saved())
            .page(ctx)
        }
        TasksState::Ready(tasks) if tasks.tasks.is_empty() => start::start(ctx, &project, 0),
        TasksState::Ready(tasks) => body(ctx, &project, tasks, &around),
        TasksState::Loading => reading(ctx, &project, &around),
      });
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

/// The two columns of a project with tasks.
fn body(ctx: &mut Ctx, project: &ShownProject, tasks: &ProjectTasks, around: &Around) -> Element {
  let id = project.open.project.id();
  let root = project.open.repo_root.clone();
  let items = attention::items(tasks);
  let at_work = runs_at_work(tasks);
  let slots_full = around
    .slots
    .is_some_and(|(working, slots)| working >= usize::try_from(slots.get()).unwrap_or(usize::MAX));
  let queued = queued_tasks(tasks, slots_full);
  let merges = merged(tasks, around.now.local_day_start());
  let mut main = Column::new()
    .id("overview-main")
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Xl);
  if !items.is_empty() {
    main = main.child(digest::digest(ctx, id, &items, around.now));
  }
  if items.is_empty() && at_work.is_empty() && queued.is_empty() {
    let last_merge = tasks.run_facts.merged_at.values().max().copied();
    main = main
      .child(quiet::nothing_waits(ctx, around.slots, last_merge))
      .with_children(quiet::ready_to_start(ctx, id, tasks));
  }
  if !at_work.is_empty() {
    main = main.child(at_work::at_work(
      ctx,
      id,
      (&at_work, &around.accounts),
      (around.slots, around.now),
    ));
  }
  if !queued.is_empty() || tasks.queue.project {
    main = main.child(queue::queue(ctx, (id, root.clone()), &queued, tasks.queue.project));
  }
  main = main.with_children(notices::unreadable(ctx, &root, tasks));
  let account_ids: Vec<_> = around.accounts.0.iter().map(|account| account.id()).collect();
  let near = near_limits(&account_ids, &around.readings, around.now);
  let side = side_column()
    .with_children(limits::limits(ctx, (&near, &around.accounts), &at_work, around.now))
    .child(landed::landed(ctx, id, (&merges, queue_line(tasks))))
    .child(delivery::delivery(ctx, id, root, project.feed.saves.get()));
  columns(main, side)
}

/// The page while the tasks are read: the sections' heads with bones for their rows, the plan limits from memory.
fn reading(ctx: &mut Ctx, project: &ShownProject, around: &Around) -> Element {
  let id = project.open.project.id();
  let account_ids: Vec<_> = around.accounts.0.iter().map(|account| account.id()).collect();
  let near = near_limits(&account_ids, &around.readings, around.now);
  let row_bones = |id: &'static str, titles| BonesProps {
    id,
    height: section::LINE_HEIGHT,
    titles,
  };
  let main = Column::new()
    .id("overview-main")
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Xl)
    .child(
      digest::frame("overview-waits")
        .child(digest::reading_head(ctx))
        .child(bones(ctx, row_bones("overview-waits-bones", WAIT_BONES))),
    )
    .child(
      section::section("overview-at-work")
        .child(section::head(
          "overview-at-work",
          &ctx.t("overview.atWork.title"),
          None,
          None,
        ))
        .child(bones(ctx, row_bones("overview-at-work-bones", RUN_BONES))),
    )
    .child(
      section::section("overview-queue")
        .child(section::head(
          "overview-queue",
          &ctx.t("overview.queue.title"),
          None,
          None,
        ))
        .child(bones(ctx, row_bones("overview-queue-bones", RUN_BONES))),
    );
  let side_bones = BonesProps {
    id: "overview-landed-bones",
    height: section::SIDE_LINE_HEIGHT,
    titles: SIDE_BONES,
  };
  let side = side_column()
    .with_children(limits::limits(ctx, (&near, &around.accounts), &[], around.now))
    .child(landed::reading(ctx).child(bones(ctx, side_bones)))
    .child(delivery::delivery(
      ctx,
      id,
      project.open.repo_root.clone(),
      project.feed.saves.get(),
    ));
  columns(main, side)
}

fn side_column() -> Column {
  Column::new()
    .id("overview-side")
    .width(Dimension::Px(SIDE_WIDTH))
    .flex_shrink(0.0)
    .spacing(SpacingSize::Xl)
}

fn columns(main: Column, side: Column) -> Element {
  Row::new()
    .id("overview-columns")
    .width(Dimension::Pct(100.0))
    .spacing(OverviewSpacing::Columns.role())
    .align_items(Alignment::Start)
    .child(main)
    .child(side)
    .into()
}
