//! The sidebar's *Running* list (`Section · Running` `o0tXx` and `Run item` `mONDf` in `q9mf0k`): each
//! task whose latest run is under way, as its key and the step its agent works on, and how long the agent
//! has worked on the step. A run that waits for the person shows an amber hand (`C2E5Po`); while no runner
//! looks after the project's agents, a run recorded as running is stopped, with no time (`KGpMB`). The
//! section shows only while a run is under way; its list icon opens the Overview's Running tab, a row the
//! task's conversation.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::{ProjectId, RunId, TaskId},
  project::TaskKey,
  run::{ActiveClock, TaskPhase},
  store::RunStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use orchester_workspace::{
  background, messages,
  tasks::{
    data::ProjectTasks,
    detail::{run_store_error, run_view},
  },
};

use super::section_label;
use crate::{
  navigation::Navigation,
  routes::{OverviewTab, ProjectPage, Route, TaskView},
  shell::clock::{Clock, clock},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    nav_item::NAV_ITEM_HEIGHT,
    pressable::opening,
  },
};

/// A task whose latest run is under way, as the list shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunningRow {
  pub key: TaskKey,
  pub state: RowState,
  /// The step its agent works on, once the run was read.
  pub step: Option<Arc<str>>,
  /// The agent's active time on that step, once the run was read; none while the run is stopped.
  pub clock: Option<ActiveClock>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowState {
  /// An agent works.
  Working,
  /// It waits for the person.
  NeedsYou,
  /// It was recorded as running, but no runner looks after its agent.
  Stopped,
}

impl DevtoolsInspectable for RunningRow {}

/// The tasks of `tasks` whose latest run is under way, in the list's order, with the run, its phase and
/// when it last changed (which decides when it is read again). `online`: a runner looks after the agents.
pub fn under_way(tasks: &ProjectTasks, online: bool) -> Vec<UnderWay> {
  tasks
    .tasks
    .iter()
    .filter_map(|task| {
      let latest = tasks.phases.get(&task.id())?.run()?;
      let state = match (latest.phase, online) {
        (TaskPhase::Running, true) => RowState::Working,
        (TaskPhase::Running, false) => RowState::Stopped,
        (TaskPhase::Waiting, _) => RowState::NeedsYou,
        _ => return None,
      };
      Some(UnderWay {
        task: task.id(),
        key: task.key().clone(),
        run: latest.id,
        state,
        updated_at: latest.updated_at,
      })
    })
    .collect()
}

/// A run under way, before its steps are read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnderWay {
  pub task: TaskId,
  pub key: TaskKey,
  pub run: RunId,
  pub state: RowState,
  pub updated_at: Timestamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunningProps {
  pub project: ProjectId,
  pub repo_root: PathBuf,
  pub runs: Vec<UnderWay>,
}

impl DevtoolsInspectable for RunningProps {}
impl DevtoolsInspectable for UnderWay {}

/// Reads the steps of the runs under way and shows the list; nothing while none is.
pub struct Running;

impl Component for Running {
  type Props = RunningProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let RunningProps {
      project,
      repo_root,
      runs,
    } = ctx.props::<RunningProps>().clone();
    let read = ctx.future((repo_root, runs.clone()), |(repo_root, runs)| async move {
      background::run(move || read_rows(&repo_root, &runs))
        .await?
        .map_err(|error| {
          tracing::error!(%error, "cannot read the runs under way");
          messages::store_error(&error)
        })
    });
    if runs.is_empty() {
      return Element::new();
    }
    // Until the steps are read, and if they cannot be, the rows say what the tasks say.
    let rows = read.state().get().data.unwrap_or_else(|| {
      runs
        .iter()
        .map(|run| RunningRow {
          key: run.key.clone(),
          state: run.state,
          step: None,
          clock: None,
        })
        .collect()
    });
    running_list(ctx, project, &rows)
  }
}

/// Each run's current step and its agent's active time on it, read from the project's data.
fn read_rows(repo_root: &Path, runs: &[UnderWay]) -> Result<Vec<RunningRow>, StoreError> {
  if runs.is_empty() {
    return Ok(Vec::new());
  }
  let db = ProjectDb::open(repo_root)?;
  let now = Timestamp::now();
  runs
    .iter()
    .map(|under_way| {
      let mut row = RunningRow {
        key: under_way.key.clone(),
        state: under_way.state,
        step: None,
        clock: None,
      };
      let Some(run) = db.run(under_way.run).map_err(run_store_error)? else {
        return Ok(row);
      };
      let view = run_view(&db, run)?;
      if let Some(step) = view.current_step() {
        row.step = Some(step.name.clone());
        let lost = under_way.state == RowState::Stopped;
        row.clock = step
          .execution
          .filter(|_| !lost)
          .map(|execution| view.active_clock(view.invocations_of(execution), now, lost));
      }
      Ok(row)
    })
    .collect()
}

/// The section and its rows, with the element id `running`.
pub fn running_list(ctx: &mut Ctx, project: ProjectId, rows: &[RunningRow]) -> Element {
  let navigation = Navigation::of(ctx);
  let header = section_label(&ctx.t("shell.running.title")).child(
    opening(navigation.to(Route::Project(project, ProjectPage::Overview(OverviewTab::Running))))
      .id("running-all")
      .describe("a11y_name", ctx.t("shell.running.all"))
      .rounded(RadiusSize::Sm)
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .child(icon(Icon::List, IconSize::Sm, PaletteColor::TextMuted)),
  );
  let list = rows.iter().fold(
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Tight.role()),
    |list, row| list.child(running_row(ctx, &navigation, project, row)),
  );
  Column::new()
    .id("running")
    .width(Dimension::Pct(100.0))
    .child(header)
    .child(list)
    .into()
}

/// A row (`Run item` `mONDf`): its state's icon, *KON-12 · Implement* and *11m*.
fn running_row(ctx: &mut Ctx, navigation: &Navigation, project: ProjectId, row: &RunningRow) -> Element {
  let (glyph, color, state) = match row.state {
    RowState::Working => (Icon::LoaderCircle, PaletteColor::TextSecondary, "working"),
    RowState::NeedsYou => (Icon::Hand, PaletteColor::Warning, "needs-you"),
    RowState::Stopped => (Icon::CirclePause, PaletteColor::Warning, "stopped"),
  };
  let key = row.key.to_string();
  let label = match (row.state, &row.step) {
    (RowState::Stopped, _) => ctx.t_args("shell.running.stopped", [("key", key.as_str())]),
    (_, Some(step)) => ctx.t_args("shell.running.step", [("key", key.as_str()), ("step", step)]),
    (_, None) => key.as_str().into(),
  };
  let id = format!("running-{key}");
  let time = match row.clock {
    Some(active) => clock(ctx, &format!("{id}-time"), Clock::Active(active)),
    None => Text::new(&ctx.t("shell.running.noTime"))
      .id(format!("{id}-time"))
      .variant(ExtraTypography::MonoNote.role())
      .into(),
  };
  let route = Route::Project(project, ProjectPage::Task(row.key.clone(), TaskView::Conversation));
  opening(navigation.to(route))
    .id(id)
    .describe("state", state)
    .describe("a11y_name", label.clone())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(NAV_ITEM_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(ExtraSpacing::Inline.role())
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(icon(glyph, IconSize::Md, color))
    .child(
      Row::new().flex(1.0).min_width(Dimension::Px(0.0)).child(
        Text::new(&label)
          .variant(TypographyStyle::Description)
          .nowrap()
          .text_overflow(TextOverflow::Elipsis),
      ),
    )
    .child(time)
    .align_items(Alignment::Center)
    .into()
}
