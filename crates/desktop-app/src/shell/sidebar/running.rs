//! The sidebar's *Running* list (`Running head` `veG7t` and `Running row` `MDsM7` in `Zn8dR`): its head says
//! how many runs are under way, then each task whose latest run is under way, as its key and the step its agent
//! works on, and how long the agent has worked on the step. A run that waits for the person shows an amber hand
//! (`C2E5Po`); while no runner looks after the project's agents, a run recorded as running is stopped, with no time
//! (`KGpMB`). The section shows only while a run is under way (`DE34j` has none); a row opens the task's
//! conversation.

use std::{path::PathBuf, sync::Arc};

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
  id::ProjectId,
  project::TaskKey,
  run::{ActiveClock, TaskPhase},
};
use orchester_workspace::{
  background, messages,
  tasks::active_runs::{self, ActiveRun},
};

use super::{section_gap, section_head};
use crate::{
  navigation::Navigation,
  project::{Projects, settled_read::settled_read},
  routes::{ProjectPage, Route, TaskView},
  services::Services,
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

/// The rows of `runs`, the runs under way in the list's order. `online`: a runner looks after the agents; without
/// one a run recorded as running has lost its agent, is shown stopped and has no time.
pub fn rows(runs: &[ActiveRun], online: bool) -> Vec<RunningRow> {
  runs
    .iter()
    .map(|run| {
      let state = match (run.phase, online) {
        (TaskPhase::Waiting, _) => RowState::NeedsYou,
        (_, true) => RowState::Working,
        (_, false) => RowState::Stopped,
      };
      RunningRow {
        key: run.key.clone(),
        state,
        step: run.step.clone(),
        clock: run.clock.filter(|_| state != RowState::Stopped),
      }
    })
    .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunningProps {
  pub project: ProjectId,
  pub repo_root: PathBuf,
}

impl DevtoolsInspectable for RunningProps {}

/// Reads the runs under way and shows the list; nothing until they are read and while none is. The runs are read
/// alone, again when a run changes state or the project's tasks are saved, never for a transcript's lines, and never
/// beside a reading that has not ended (`settled_read`).
pub struct Running;

impl Component for Running {
  type Props = RunningProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let RunningProps { project, repo_root } = ctx.props::<RunningProps>().clone();
    let feed = Projects::of(ctx).feed(project);
    let online = feed.runner_online(&repo_root);
    let deps = (repo_root, feed.states.get(), feed.saves.get());
    let hold = Services::of(ctx).hold;
    let read = settled_read(ctx, "running", deps, move |(repo_root, ..)| {
      let hold = hold.clone();
      async move {
        hold.released().await;
        background::run(move || active_runs::load(&repo_root))
          .await?
          .map_err(|error| {
            tracing::error!(%error, "cannot read the runs under way");
            messages::store_error(&error)
          })
      }
    });
    // The runs last read stay while they are read again, and none is shown before the first reading: the count is
    // never a guess.
    let Some(runs) = read.data.filter(|runs| !runs.is_empty()) else {
      return Element::new();
    };
    running_list(ctx, project, &rows(&runs, online))
  }
}

/// The section's gap, its head with how many runs are under way, and its rows, with the element id `running`.
pub fn running_list(ctx: &mut Ctx, project: ProjectId, rows: &[RunningRow]) -> Element {
  let navigation = Navigation::of(ctx);
  let header = section_head(&ctx.t("shell.running.title"), ExtraSpacing::Snug.role()).child(
    Text::new(&rows.len().to_string())
      .id("running-count")
      .variant(TypographyStyle::Caption)
      .nowrap(),
  );
  rows
    .iter()
    .fold(
      Column::new()
        .id("running")
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(section_gap())
        .child(header),
      |list, row| list.child(running_row(ctx, &navigation, project, row)),
    )
    .into()
}

/// A row (`Running row` `MDsM7`): its state's icon, *KON-12 · Implement* and *11m*.
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
