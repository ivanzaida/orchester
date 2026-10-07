//! *Most expensive* (`C2pTYF`, `sOQmy`): the tasks or the runs of the period by spent tokens. *Tasks*:
//! task (key in mono, title), pipeline, runs, spent tokens, cached input, outcome (`DeizF`) and a
//! chevron; a row opens the task. *Runs*: run (*ORC-128 · Run 3* in mono, task title), pipeline,
//! started, agent time, spent tokens, cached input, outcome; a row opens the run's task. Across
//! projects the table adds *Project*. Five rows, then *Show all*.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Stack, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, Style, dimension::Dimension},
};
use orchester_core::{
  id::{ProjectId, TaskId},
  usage::{RunCost, RunOutcome},
};

use super::{
  card::{Card, text_link},
  expensive_columns::{COUNT_WIDTH, Fixed, Layout, TIME_WIDTH, TOKENS_WIDTH, columns, outcome_glyph},
  format,
  measure::Measurer,
  state::{Dashboard, Views},
  table::{Cell, TableColumn, Width, column, fit, row, spaced_table},
};
use crate::{
  background,
  messages::{self, Message},
  navigation::{Navigation, route::Page},
  screens::project::ProjectNav,
  tabs::OpenIn,
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    icon::{Icon, IconSize, icon},
    link::{Link, Press},
    segments::{SegmentSize, segments},
    status_label::compact_status_label,
  },
  workspace::OpenProject,
};

/// Rows before *Show all*.
const FIRST_ROWS: usize = 5;
#[derive(Clone, Debug, PartialEq, Eq)]
struct Opened(OpenProject, Page, OpenIn);

impl lurq::app::component::DevtoolsInspectable for Opened {}

type OpenAction = FutureAction<(PathBuf, Page, OpenIn), Opened, Message>;

#[derive(Clone)]
pub struct ExpensiveProps {
  pub dashboard: Arc<Dashboard>,
  pub views: Signal<Views>,
}

impl PartialEq for ExpensiveProps {
  fn eq(&self, other: &Self) -> bool {
    self.dashboard == other.dashboard
  }
}

impl lurq::app::component::DevtoolsInspectable for ExpensiveProps {}

pub struct ExpensiveTable {
  measurer: Measurer,
}

impl Component for ExpensiveTable {
  type Props = ExpensiveProps;

  fn create(ctx: &mut Ctx) -> Self {
    let action = open_action(ctx);
    let navigation = navigation(ctx);
    ctx.watch(&action.state(), move |state| {
      if let Some(Opened(project, page, open)) = state.data.clone().filter(|_| !state.is_pending()) {
        navigation.open_project_page(project, page, open);
      }
    });
    Self {
      measurer: Measurer::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let action = open_action(ctx);
    let props = ctx.props::<ExpensiveProps>().clone();
    let views = props.views.get();
    let signal = props.views.clone();
    let switch = segments(
      "usage-expensive-mode",
      &views.runs,
      vec![
        (false, ctx.t("usage.expensive.tasks")),
        (true, ctx.t("usage.expensive.runs")),
      ],
      SegmentSize::Compact,
      move |runs| signal.update(|views| views.runs = runs),
    );
    let opener = Opener {
      nav: ctx.use_context::<ProjectNav>(),
      navigation: navigation(ctx),
      dashboard: Arc::clone(&props.dashboard),
      action,
    };
    let table = if views.runs {
      runs(ctx, &props.dashboard, (&opener, &self.measurer), views.all_rows)
    } else {
      tasks(ctx, &props.dashboard, (&opener, &self.measurer), views.all_rows)
    };
    let body = Stack::new()
      .width(Dimension::Pct(100.0))
      .child(table)
      .child(self.measurer.canvas())
      .into();
    let shown = if views.runs {
      props.dashboard.report.runs.len()
    } else {
      props.dashboard.report.tasks.len()
    };
    let note = Text::new(&ctx.t("usage.expensive.note"))
      .variant(TypographyStyle::Caption)
      .color(PaletteColor::TextMuted)
      .nowrap();
    Card::new("usage-expensive", ctx.t("usage.expensive.title"))
      .lead(switch)
      .trailing(note.into())
      .body([body, footer(ctx, &props.views, views, shown)])
  }

  fn after_layout(&self) {
    self.measurer.after_layout();
  }
}

fn navigation(ctx: &mut Ctx) -> Navigation {
  ctx
    .use_context::<Navigation>()
    .expect("the root provides Navigation before mounting screens")
}

fn open_action(ctx: &mut Ctx) -> OpenAction {
  let workspace = navigation(ctx).workspace().clone();
  ctx.future_action(move |(root, page, open): (PathBuf, Page, OpenIn)| {
    let workspace = workspace.clone();
    async move {
      background::run(move || workspace.open(&root))
        .await?
        .map(|project| Opened(project, page, open))
        .map_err(|error| {
          tracing::error!(%error, "cannot open the project of a listed task");
          messages::store_error(&error)
        })
    }
  })
}

/// Opens a listed task: in the project shown through its navigation, across projects after opening
/// the task's project.
struct Opener {
  nav: Option<ProjectNav>,
  navigation: Navigation,
  dashboard: Arc<Dashboard>,
  action: OpenAction,
}

impl Opener {
  fn link(&self, project: ProjectId, task: TaskId) -> Option<Link> {
    let page = Page::task(task);
    if let Some(nav) = self.nav.as_ref().filter(|_| !self.dashboard.all_projects()) {
      return Some(nav.link(page));
    }
    let root = self.dashboard.roots.get(&project)?.clone();
    let action = self.action.clone();
    Some(Link::new(self.navigation.tabs(), move |open| {
      action.run((root.clone(), page, open))
    }))
  }
}

fn outcome(ctx: &Ctx, outcome: RunOutcome) -> Element {
  status(outcome, &ctx.t(format::outcome_key(outcome)))
}

fn status(outcome: RunOutcome, label: &str) -> Element {
  compact_status_label(outcome_glyph(outcome), label)
}

/// The key in mono and `muted`, and the title beside it, or under it in the narrow layout
/// (`sOQmy`); the title gives way with an ellipsis, the key keeps its width beside it.
fn named(id: String, key: &str, title: &str, stacked: bool) -> Element {
  let title = fit(Text::new(title).id(id).variant(TypographyStyle::Body));
  let key = Text::new(key)
    .variant(ExtraTypography::MonoValue.role())
    .color(PaletteColor::TextSecondary)
    .nowrap();
  if stacked {
    return Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Hair.role())
      .child(fit(key))
      .child(title)
      .into();
  }
  Row::new()
    .flex_shrink(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(key.flex_shrink(0.0))
    .child(title)
    .into()
}

fn chevron() -> Cell {
  Cell::Element(icon(Icon::ChevronRight, IconSize::Small, PaletteColor::TextMuted).into())
}

/// `columns`, with the project's after the first across projects (`p4eVV6`).
fn with_project(ctx: &Ctx, dashboard: &Dashboard, mut columns: Vec<TableColumn>) -> Vec<TableColumn> {
  if dashboard.all_projects() {
    columns.insert(1, column(ctx.t("usage.table.project"), Width::Flex(0.8), false));
  }
  columns
}

/// `cells`, with the project's after the first across projects.
fn cells(dashboard: &Dashboard, project: &Arc<str>, mut cells: Vec<Cell>) -> Vec<Cell> {
  if dashboard.all_projects() {
    cells.insert(1, Cell::Muted(project.clone()));
  }
  cells
}

fn tasks(ctx: &Ctx, dashboard: &Dashboard, (opener, measurer): (&Opener, &Measurer), all: bool) -> Element {
  let layout = Layout::of(ctx);
  let shown = &dashboard.report.tasks[..dashboard
    .report
    .tasks
    .len()
    .min(if all { usize::MAX } else { FIRST_ROWS })];
  let flexible = vec![
    column(ctx.t("usage.table.task"), Width::Flex(2.4), false),
    column(ctx.t("usage.table.pipeline"), Width::Flex(1.2), false),
  ];
  let fixed = vec![
    Fixed::figures(
      ctx,
      ctx.t("usage.summary.runs"),
      COUNT_WIDTH,
      shown.iter().map(|task| task.runs.to_string().into()),
    ),
    Fixed::figures(
      ctx,
      ctx.t("usage.table.spent"),
      TOKENS_WIDTH,
      shown.iter().map(|task| format::tokens(task.usage.counted())),
    ),
    Fixed::figures(
      ctx,
      ctx.t("usage.table.cached"),
      TOKENS_WIDTH,
      shown.iter().map(|task| format::tokens(task.usage.cached_input)),
    ),
    Fixed::outcome(ctx, shown.iter().map(|task| task.outcome)),
    Fixed::chevron(ctx),
  ];
  let columns = columns(ctx, (layout, measurer), with_project(ctx, dashboard, flexible), fixed);
  let rows = shown
    .iter()
    .enumerate()
    .map(|(index, task)| {
      let title = format!("usage-expensive-title-{index}");
      let cells = cells(
        dashboard,
        &task.project_name,
        vec![
          Cell::Element(named(title, &task.key, &task.title, false)),
          pipeline(ctx, task.pipeline.as_ref()),
          Cell::Text(task.runs.to_string().into()),
          Cell::Text(format::tokens(task.usage.counted())),
          Cell::Text(format::tokens(task.usage.cached_input)),
          Cell::Element(outcome(ctx, task.outcome)),
          chevron(),
        ],
      );
      let id = format!("usage-expensive-tasks-row-{index}");
      pressed(
        row(&id, &columns, cells, layout.gap()),
        opener.link(task.project, task.task),
      )
    })
    .collect();
  spaced_table("usage-expensive-tasks", &columns, rows, layout.gap())
}

fn runs(ctx: &Ctx, dashboard: &Dashboard, (opener, measurer): (&Opener, &Measurer), all: bool) -> Element {
  let layout = Layout::of(ctx);
  let shown = &dashboard.report.runs[..dashboard
    .report
    .runs
    .len()
    .min(if all { usize::MAX } else { FIRST_ROWS })];
  // Narrow, the title sits under its key, so the pipeline can take a little more of the room.
  let (run_share, pipeline_share) = if layout.stacked() { (2.1, 1.2) } else { (2.4, 1.1) };
  let flexible = vec![
    column(ctx.t("usage.table.run"), Width::Flex(run_share), false),
    column(ctx.t("usage.table.pipeline"), Width::Flex(pipeline_share), false),
  ];
  let columns = columns(
    ctx,
    (layout, measurer),
    with_project(ctx, dashboard, flexible),
    run_columns(ctx, shown),
  );
  let rows = shown
    .iter()
    .enumerate()
    .map(|(index, run)| {
      let label = ctx.t_args(
        "usage.expensive.runLabel",
        [("task", &*run.key), ("run", run.number.get().to_string().as_str())],
      );
      let title = format!("usage-expensive-title-{index}");
      let cells = cells(
        dashboard,
        &run.project_name,
        vec![
          Cell::Element(named(title, &label, &run.title, layout.stacked())),
          pipeline(ctx, run.pipeline.as_ref()),
          Cell::Muted(format::date_time(ctx, run.started)),
          Cell::Text(format::duration(ctx, run.agent_ms)),
          Cell::Text(format::tokens(run.usage.counted())),
          Cell::Text(format::tokens(run.usage.cached_input)),
          Cell::Element(outcome(ctx, run.outcome)),
          chevron(),
        ],
      );
      let id = format!("usage-expensive-runs-row-{index}");
      pressed(
        row(&id, &columns, cells, layout.gap()),
        opener.link(run.project, run.task),
      )
    })
    .collect();
  spaced_table("usage-expensive-runs", &columns, rows, layout.gap())
}

/// The fixed columns of *Runs*: started, agent time, spent tokens, cached input, outcome, chevron.
fn run_columns(ctx: &Ctx, shown: &[RunCost]) -> Vec<Fixed> {
  vec![
    Fixed::text(
      ctx,
      ctx.t("usage.table.started"),
      TIME_WIDTH,
      shown.iter().map(|run| format::date_time(ctx, run.started)),
    ),
    Fixed::figures(
      ctx,
      ctx.t("usage.summary.agentTime"),
      TIME_WIDTH,
      shown.iter().map(|run| format::duration(ctx, run.agent_ms)),
    ),
    Fixed::figures(
      ctx,
      ctx.t("usage.table.spent"),
      TOKENS_WIDTH,
      shown.iter().map(|run| format::tokens(run.usage.counted())),
    ),
    Fixed::figures(
      ctx,
      ctx.t("usage.table.cached"),
      TOKENS_WIDTH,
      shown.iter().map(|run| format::tokens(run.usage.cached_input)),
    ),
    Fixed::outcome(ctx, shown.iter().map(|run| run.outcome)),
    Fixed::chevron(ctx),
  ]
}

/// The pipeline a task or run ran on, or that it is gone.
fn pipeline(ctx: &Ctx, pipeline: Option<&Arc<str>>) -> Cell {
  Cell::Muted(
    pipeline
      .cloned()
      .unwrap_or_else(|| ctx.t("usage.breakdown.pipelineGone")),
  )
}

/// A row that opens its task, with the hover fill.
fn pressed(row: Row, link: Option<Link>) -> Row {
  let Some(link) = link else {
    return row;
  };
  Row::new().width(Dimension::Pct(100.0)).child(
    link.on(
      crate::ui::pressable::surface()
        .width(Dimension::Pct(100.0))
        .hovered_style(Style::new().background(PaletteColor::AccentMuted))
        .child(row),
    ),
  )
}

/// *5 of 38 tasks with runs in the period* and *Show all 38 →*, or back to the first five.
fn footer(ctx: &Ctx, signal: &Signal<Views>, views: Views, count: usize) -> Element {
  let shown = if views.all_rows { count } else { count.min(FIRST_ROWS) };
  let text = ctx.t_args(
    if views.runs {
      "usage.expensive.runsShown"
    } else {
      "usage.expensive.tasksShown"
    },
    [
      ("shown", shown.to_string().as_str()),
      ("count", count.to_string().as_str()),
    ],
  );
  let signal = signal.clone();
  let toggle = (count > FIRST_ROWS).then(|| {
    let label = if views.all_rows {
      ctx.t_args(
        "usage.expensive.showFirst",
        [("count", FIRST_ROWS.to_string().as_str())],
      )
    } else {
      ctx.t_args("usage.expensive.showAll", [("count", count.to_string().as_str())])
    };
    text_link(
      "usage-expensive-all",
      &label,
      Press::action(move || signal.update(|views| views.all_rows = !views.all_rows)),
    )
  });
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .child(Text::new(&text).variant(TypographyStyle::Caption))
    .child(Spacer::new().flex(1.0))
    .with_children(toggle)
    .into()
}

pub fn expensive(ctx: &mut Ctx, dashboard: &Arc<Dashboard>, views: &Signal<Views>) -> Element {
  ctx.mount_keyed::<ExpensiveTable>(
    "usage-expensive",
    ExpensiveProps {
      dashboard: Arc::clone(dashboard),
      views: views.clone(),
    },
  )
}
