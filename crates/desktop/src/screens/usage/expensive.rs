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
    theme::{Breakpoint, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, Style, dimension::Dimension},
};
use orchester_core::{
  id::{ProjectId, TaskId},
  usage::RunOutcome,
};

use super::{
  card::{Card, text_link},
  format,
  state::{Dashboard, Views},
  table::{Cell, Sizer, TableColumn, Width, column, fit, row, size_to, spaced_table},
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
/// Fixed column widths (`sOQmy`): a count, a token figure, a time, the outcome label, the chevron.
const COUNT_WIDTH: f32 = 56.0;
const TOKENS_WIDTH: f32 = 108.0;
const TIME_WIDTH: f32 = 104.0;
const OUTCOME_WIDTH: f32 = 132.0;
const CHEVRON_WIDTH: f32 = 16.0;

/// How the table lays out: as `sOQmy` draws it at lurq's widest breakpoint, or narrower, which the
/// design does not draw, with each fixed column as wide as its heading and longest cell, the
/// columns 8 apart and a run's title under its key, so titles and pipelines keep readable room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Layout {
  Wide,
  Narrow,
}

impl Layout {
  fn of(ctx: &Ctx) -> Self {
    if matches!(ctx.breakpoint(), Some(Breakpoint::Xl)) {
      Self::Wide
    } else {
      Self::Narrow
    }
  }

  fn gap(self) -> SpacingSize {
    match self {
      Self::Wide => SpacingSize::Md,
      Self::Narrow => SpacingSize::Sm,
    }
  }

  /// A fixed column, `width` wide as designed or fitted to its heading and cells.
  fn fixed(self, heading: Arc<str>, width: f32, figures: bool) -> TableColumn {
    match self {
      Self::Wide => column(heading, Width::Px(width), figures),
      Self::Narrow => column(heading, Width::Fit, figures),
    }
  }

  /// The *Outcome* column, fitted to the longest of `outcomes`' labels.
  fn outcome(self, ctx: &Ctx, outcomes: impl Iterator<Item = RunOutcome>) -> TableColumn {
    let mut outcome = self.fixed(ctx.t("usage.table.outcome"), OUTCOME_WIDTH, false);
    if self == Self::Narrow {
      let longest = outcomes
        .map(|outcome| (outcome, ctx.t(format::outcome_key(outcome))))
        .max_by_key(|(_, label)| label.chars().count());
      outcome.sizer = longest.map(|(outcome, label)| Sizer::new(move || status(outcome, &label)));
    }
    outcome
  }
}

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

pub struct ExpensiveTable;

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
    Self
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
    let body = if views.runs {
      runs(ctx, &props.dashboard, &opener, views.all_rows)
    } else {
      tasks(ctx, &props.dashboard, &opener, views.all_rows)
    };
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
  let glyph = match outcome {
    RunOutcome::Pending => Icon::Clock3,
    RunOutcome::Running => Icon::Loader,
    RunOutcome::Paused | RunOutcome::Interrupted => Icon::CirclePause,
    RunOutcome::Waiting => Icon::MessageCircle,
    RunOutcome::Succeeded => Icon::CircleCheck,
    RunOutcome::Failed => Icon::CircleX,
    RunOutcome::Cancelled => Icon::CircleSlash,
  };
  compact_status_label(glyph, label)
}

/// The key in mono and the title beside it, or under it in the narrow layout; the title gives way
/// with an ellipsis, the key keeps its width beside it.
fn named(id: String, key: &str, title: &str, stacked: bool) -> Element {
  let title = fit(
    Text::new(title)
      .id(id)
      .variant(TypographyStyle::Body)
      .color(PaletteColor::TextSecondary),
  );
  let key = Text::new(key).variant(ExtraTypography::MonoValue.role()).nowrap();
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
fn columns(ctx: &Ctx, dashboard: &Dashboard, mut columns: Vec<TableColumn>) -> Vec<TableColumn> {
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

fn tasks(ctx: &Ctx, dashboard: &Dashboard, opener: &Opener, all: bool) -> Element {
  let layout = Layout::of(ctx);
  let shown = &dashboard.report.tasks[..dashboard
    .report
    .tasks
    .len()
    .min(if all { usize::MAX } else { FIRST_ROWS })];
  let mut columns = columns(
    ctx,
    dashboard,
    vec![
      column(ctx.t("usage.table.task"), Width::Flex(2.4), false),
      column(ctx.t("usage.table.pipeline"), Width::Flex(1.2), false),
      layout.fixed(ctx.t("usage.summary.runs"), COUNT_WIDTH, true),
      layout.fixed(ctx.t("usage.table.spent"), TOKENS_WIDTH, true),
      layout.fixed(ctx.t("usage.table.cached"), TOKENS_WIDTH, true),
      layout.outcome(ctx, shown.iter().map(|task| task.outcome)),
      layout.fixed(Arc::from(""), CHEVRON_WIDTH, false),
    ],
  );
  let rows: Vec<Vec<Cell>> = shown
    .iter()
    .enumerate()
    .map(|(index, task)| {
      let title = format!("usage-expensive-title-{index}");
      cells(
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
      )
    })
    .collect();
  size_to(&mut columns, &rows);
  let rows = rows
    .into_iter()
    .zip(shown)
    .map(|(cells, task)| pressed(row(&columns, cells, layout.gap()), opener.link(task.project, task.task)))
    .collect();
  spaced_table("usage-expensive-tasks", &columns, rows, layout.gap())
}

fn runs(ctx: &Ctx, dashboard: &Dashboard, opener: &Opener, all: bool) -> Element {
  let layout = Layout::of(ctx);
  let shown = &dashboard.report.runs[..dashboard
    .report
    .runs
    .len()
    .min(if all { usize::MAX } else { FIRST_ROWS })];
  let mut columns = columns(
    ctx,
    dashboard,
    vec![
      column(ctx.t("usage.table.run"), Width::Flex(2.4), false),
      column(ctx.t("usage.table.pipeline"), Width::Flex(1.1), false),
      layout.fixed(ctx.t("usage.table.started"), TIME_WIDTH, false),
      layout.fixed(ctx.t("usage.summary.agentTime"), TIME_WIDTH, true),
      layout.fixed(ctx.t("usage.table.spent"), TOKENS_WIDTH, true),
      layout.fixed(ctx.t("usage.table.cached"), TOKENS_WIDTH, true),
      layout.outcome(ctx, shown.iter().map(|run| run.outcome)),
      layout.fixed(Arc::from(""), CHEVRON_WIDTH, false),
    ],
  );
  let rows: Vec<Vec<Cell>> = shown
    .iter()
    .enumerate()
    .map(|(index, run)| {
      let label = ctx.t_args(
        "usage.expensive.runLabel",
        [("task", &*run.key), ("run", run.number.get().to_string().as_str())],
      );
      let title = format!("usage-expensive-title-{index}");
      cells(
        dashboard,
        &run.project_name,
        vec![
          Cell::Element(named(title, &label, &run.title, layout == Layout::Narrow)),
          pipeline(ctx, run.pipeline.as_ref()),
          Cell::Muted(format::date_time(ctx, run.started)),
          Cell::Text(format::duration(ctx, run.agent_ms)),
          Cell::Text(format::tokens(run.usage.counted())),
          Cell::Text(format::tokens(run.usage.cached_input)),
          Cell::Element(outcome(ctx, run.outcome)),
          chevron(),
        ],
      )
    })
    .collect();
  size_to(&mut columns, &rows);
  let rows = rows
    .into_iter()
    .zip(shown)
    .map(|(cells, run)| pressed(row(&columns, cells, layout.gap()), opener.link(run.project, run.task)))
    .collect();
  spaced_table("usage-expensive-runs", &columns, rows, layout.gap())
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
