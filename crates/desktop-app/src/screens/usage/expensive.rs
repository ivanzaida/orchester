//! *Most expensive* (`Card · Most expensive` `H5SBl`): the tasks or the runs of the period by spent tokens.
//! *Tasks* (`Expensive row/task` `hHWSp`): the task (its key in mono, its title), pipeline, runs, spent
//! tokens, cached input, outcome and a chevron; a row opens the task. *Runs* (`Expensive row/run` `Vy6LW`):
//! the run (*KON-12 · Run 3* in mono, the task's title), pipeline, started, agent time, spent tokens, cached
//! input, outcome; a row opens its task. Five rows, then *Show all*.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::ProjectId,
  project::TaskKey,
  resource::VersionNumber,
  usage::{RunCost, RunOutcome, TaskCost},
};
use orchester_workspace::usage::Dashboard;

use super::{
  Views,
  card::Card,
  table::{Cell, Density, TableColumn, Width, column, fit, opening, row, table},
  words,
};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskTab, TaskView},
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    pressable::Focus,
    status::{Status, StatusChip},
    tabs::{Segment, SegmentFace, segmented},
  },
};

/// Rows before *Show all*.
const FIRST_ROWS: usize = 5;
/// Column widths (`Expensive header/task` `C931v9`, `Expensive header/run` `eTjme`).
const PROJECT_COLUMN: f32 = 116.0;
const PIPELINE_COLUMN: f32 = 150.0;
const RUNS_COLUMN: f32 = 48.0;
const STARTED_COLUMN: f32 = 100.0;
const TIME_COLUMN: f32 = 72.0;
const TOKENS_COLUMN: f32 = 96.0;
const OUTCOME_COLUMN: f32 = 112.0;
const OPEN_COLUMN: f32 = 14.0;

pub fn card(ctx: &mut Ctx, dashboard: &Arc<Dashboard>, views: &Signal<Views>) -> Element {
  let shown = views.get();
  let segment = |id: &str, key: &str, runs: bool| {
    let views = views.clone();
    Segment::new(
      format!("usage-expensive-{id}"),
      SegmentFace::Text(ctx.t(key)),
      shown.runs == runs,
      move || {
        views.update(|views| {
          views.runs = runs;
          views.all_rows = false;
        });
      },
    )
  };
  let switch = segmented(
    "usage-expensive-mode",
    [
      segment("tasks", "usage.expensive.tasks", false),
      segment("runs", "usage.expensive.runs", true),
    ],
    Focus::Pointer,
  );
  let opener = Opener {
    navigation: Navigation::of(ctx),
  };
  let (body, count) = if shown.runs {
    (
      runs(ctx, dashboard, &opener, shown.all_rows),
      dashboard.report.runs.len(),
    )
  } else {
    (
      tasks(ctx, dashboard, &opener, shown.all_rows),
      dashboard.report.tasks.len(),
    )
  };
  Card::new("usage-expensive", ctx.t("usage.expensive.title"))
    .note(ctx.t("usage.expensive.note"))
    .trailing(switch)
    .close()
    .body([body, footer(ctx, views, shown, count)])
}

/// Opens a listed task's page.
struct Opener {
  navigation: Navigation,
}

impl Opener {
  /// What opens the task `key` of `project`; `None` for a key that names no task.
  fn open(&self, project: ProjectId, key: &str) -> Option<impl Fn() + Send + Sync + 'static> {
    let key: TaskKey = key.parse().ok()?;
    let (navigation, route) = (
      self.navigation.clone(),
      Route::Project(project, ProjectPage::Task(key, TaskView::of(TaskTab::Summary))),
    );
    Some(move || navigation.go(route.clone()))
  }
}

/// A run's outcome as the status labels name it.
const fn status(outcome: RunOutcome) -> Status {
  match outcome {
    RunOutcome::Pending => Status::Queued,
    RunOutcome::Running => Status::Running,
    RunOutcome::Paused => Status::Paused,
    RunOutcome::Waiting => Status::NeedsInput,
    RunOutcome::Interrupted => Status::Interrupted,
    RunOutcome::Succeeded => Status::Succeeded,
    RunOutcome::Failed => Status::Failed,
    RunOutcome::Cancelled => Status::Canceled,
  }
}

/// The key or run in mono `text_muted` and the title beside it, which gives way with an ellipsis.
fn named(id: String, key: &str, title: &str) -> Element {
  Row::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .clip()
    .child(
      Text::new(key)
        .variant(ExtraTypography::MonoKey.role())
        .nowrap()
        .flex_shrink(0.0),
    )
    .child(fit(Text::new(title).id(id).variant(ExtraTypography::Strong.role())))
    .into()
}

fn chevron() -> Cell {
  Cell::Element(icon(Icon::ChevronRight, IconSize::Md, PaletteColor::TextMuted).into())
}

fn pipeline(ctx: &Ctx, pipeline: Option<&Arc<str>>, version: Option<VersionNumber>) -> Cell {
  Cell::Muted(pipeline.map_or_else(
    || ctx.t("usage.breakdown.pipelineGone"),
    |pipeline| words::pipeline(ctx, pipeline, version),
  ))
}

fn first<T>(rows: &[T], all: bool) -> &[T] {
  &rows[..rows.len().min(if all { usize::MAX } else { FIRST_ROWS })]
}

/// *PROJECT* before the rest on every project's page (`rHA8G`).
fn project_column(ctx: &Ctx, dashboard: &Dashboard) -> Option<TableColumn> {
  dashboard
    .project
    .is_none()
    .then(|| column(ctx.t("usage.table.project"), Width::Px(PROJECT_COLUMN), false))
}

/// A row's project's cell on every project's page.
fn project_cell(dashboard: &Dashboard, name: &Arc<str>) -> Option<Cell> {
  dashboard.project.is_none().then(|| Cell::Text(name.clone()))
}

fn task_columns(ctx: &Ctx, dashboard: &Dashboard) -> Vec<TableColumn> {
  let mut columns: Vec<TableColumn> = project_column(ctx, dashboard).into_iter().collect();
  columns.extend([
    column(ctx.t("usage.table.task"), Width::Flex(1.0), false),
    column(ctx.t("usage.table.pipeline"), Width::Px(PIPELINE_COLUMN), false),
    column(ctx.t("usage.table.runs"), Width::Px(RUNS_COLUMN), true),
    column(ctx.t("usage.table.spent"), Width::Px(TOKENS_COLUMN), true),
    column(ctx.t("usage.table.cached"), Width::Px(TOKENS_COLUMN), true),
    column(ctx.t("usage.table.outcome"), Width::Px(OUTCOME_COLUMN), false),
    column(Arc::from(""), Width::Px(OPEN_COLUMN), false),
  ]);
  columns
}

fn tasks(ctx: &mut Ctx, dashboard: &Dashboard, opener: &Opener, all: bool) -> Element {
  let columns = task_columns(ctx, dashboard);
  let rows = first(&dashboard.report.tasks, all)
    .iter()
    .enumerate()
    .map(|(index, task)| {
      let project = project_cell(dashboard, &task.project_name);
      let pipeline = pipeline(ctx, task.pipeline.as_ref(), dashboard.task_pipeline_version(task.task));
      task_row(ctx, (&columns, index), (project, pipeline, task), opener)
    })
    .collect();
  table("usage-expensive-tasks", &columns, Density::List, rows)
}

fn task_row(
  ctx: &mut Ctx,
  (columns, index): (&[TableColumn], usize),
  (project, pipeline, task): (Option<Cell>, Cell, &TaskCost),
  opener: &Opener,
) -> Element {
  let id = format!("usage-expensive-tasks-row-{index}");
  let mut cells: Vec<Cell> = project.into_iter().collect();
  cells.extend([
    Cell::Element(named(format!("{id}-title"), &task.key, &task.title)),
    pipeline,
    Cell::Text(task.runs.to_string().into()),
    Cell::Text(words::tokens(task.usage.counted())),
    Cell::Muted(words::tokens(task.usage.cached_input)),
    Cell::Element(StatusChip::new(format!("{id}-outcome"), status(task.outcome)).render(ctx)),
    chevron(),
  ]);
  let line = row(&id, columns, Density::List, cells);
  match opener.open(task.project, &task.key) {
    Some(open) => opening(line, open),
    None => line.into(),
  }
}

fn run_columns(ctx: &Ctx, dashboard: &Dashboard) -> Vec<TableColumn> {
  let mut columns: Vec<TableColumn> = project_column(ctx, dashboard).into_iter().collect();
  columns.extend([
    column(ctx.t("usage.table.run"), Width::Flex(1.0), false),
    column(ctx.t("usage.table.pipeline"), Width::Px(PIPELINE_COLUMN), false),
    column(ctx.t("usage.table.started"), Width::Px(STARTED_COLUMN), false),
    column(ctx.t("usage.table.agentTime"), Width::Px(TIME_COLUMN), true),
    column(ctx.t("usage.table.spent"), Width::Px(TOKENS_COLUMN), true),
    column(ctx.t("usage.table.cached"), Width::Px(TOKENS_COLUMN), true),
    column(ctx.t("usage.table.outcome"), Width::Px(OUTCOME_COLUMN), false),
    column(Arc::from(""), Width::Px(OPEN_COLUMN), false),
  ]);
  columns
}

fn runs(ctx: &mut Ctx, dashboard: &Dashboard, opener: &Opener, all: bool) -> Element {
  let columns = run_columns(ctx, dashboard);
  let rows = first(&dashboard.report.runs, all)
    .iter()
    .enumerate()
    .map(|(index, run)| {
      let project = project_cell(dashboard, &run.project_name);
      let pipeline = pipeline(ctx, run.pipeline.as_ref(), dashboard.run_pipeline_version(run.run));
      run_row(ctx, (&columns, index), (project, pipeline, run), opener)
    })
    .collect();
  table("usage-expensive-runs", &columns, Density::List, rows)
}

fn run_row(
  ctx: &mut Ctx,
  (columns, index): (&[TableColumn], usize),
  (project, pipeline, run): (Option<Cell>, Cell, &RunCost),
  opener: &Opener,
) -> Element {
  let id = format!("usage-expensive-runs-row-{index}");
  let label = words::run_label(ctx, &run.key, run.number.get());
  let mut cells: Vec<Cell> = project.into_iter().collect();
  cells.extend([
    Cell::Element(named(format!("{id}-title"), &label, &run.title)),
    pipeline,
    Cell::Muted(words::weekday_time(ctx, run.started)),
    Cell::Text(words::duration(ctx, run.agent_ms)),
    Cell::Text(words::tokens(run.usage.counted())),
    Cell::Muted(words::tokens(run.usage.cached_input)),
    Cell::Element(StatusChip::new(format!("{id}-outcome"), status(run.outcome)).render(ctx)),
    chevron(),
  ]);
  let line = row(&id, columns, Density::List, cells);
  match opener.open(run.project, &run.key) {
    Some(open) => opening(line, open),
    None => line.into(),
  }
}

/// *5 of 38 tasks with runs in the period* and *Show all 38 →*, or back to the first five.
fn footer(ctx: &Ctx, views: &Signal<Views>, shown: Views, count: usize) -> Element {
  let listed = if shown.all_rows { count } else { count.min(FIRST_ROWS) };
  let text = ctx.t_args(
    if shown.runs {
      "usage.expensive.runsShown"
    } else {
      "usage.expensive.tasksShown"
    },
    [
      ("shown", listed.to_string().as_str()),
      ("count", count.to_string().as_str()),
    ],
  );
  let toggle = (count > FIRST_ROWS).then(|| {
    let label = if shown.all_rows {
      ctx.t_args(
        "usage.expensive.showFirst",
        [("count", FIRST_ROWS.to_string().as_str())],
      )
    } else {
      ctx.t_args("usage.expensive.showAll", [("count", count.to_string().as_str())])
    };
    let views = views.clone();
    ActionButton::new("usage-expensive-all", ButtonKind::Ghost, label)
      .on_click(move || views.update(|views| views.all_rows = !views.all_rows))
  });
  Row::new()
    .id("usage-expensive-footer")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(SpacingSize::Xs))
    .align_items(Alignment::Center)
    .child(
      Text::new(&text)
        .id("usage-expensive-shown")
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .child(Spacer::new().flex(1.0))
    .with_children(toggle)
    .into()
}
