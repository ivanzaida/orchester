//! The closing panel in the content panel, where the page was (`Closing panel` of `u3Fw36`, `lxm8W`,
//! `n5psM`): what closing does, the runs with an agent at work by project and git's operations under way,
//! the tasks that only wait for a person, the two choices and the actions. What it lists is read live
//! (`CloseFlow::board`): it follows the runners' reports and, each second, git and the time.

mod actions;
mod parts;

use std::time::Duration;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Interval, StreamEmitter},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, ScrollVertical},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{
  close::{Board, CloseFlow, ClosePhase, RunStands, Stage},
  runner::Runners,
  tasks::integration_jobs::BusyJob,
};
use parking_lot::Mutex;

use self::parts::{choices, head, note, waiting_note};
use super::{
  keys::Choice,
  rows::{Shown, group, jobs_group},
};
use crate::{
  services::Services,
  theme::{ExtraColor, LifecycleSpacing},
};

/// The closing panel's width (`Closing panel` `v9psPQ`).
const PANEL_WIDTH: f32 = 880.0;
/// How often the panel looks again at git and says the time again.
const TICK: Duration = Duration::from_secs(1);

#[derive(Clone)]
pub struct PanelProps {
  pub phase: ClosePhase,
  pub close: CloseFlow,
  /// The choice the keyboard moved to.
  pub focus: Signal<Option<Choice>>,
}

impl PartialEq for PanelProps {
  fn eq(&self, other: &Self) -> bool {
    self.phase == other.phase && self.close == other.close
  }
}

impl DevtoolsInspectable for PanelProps {}

pub struct ClosingPanel {
  tick: Signal<u64>,
  interval: Interval,
  buttons: [(Choice, ElementRef); 3],
  /// The choice the focus was last moved to.
  focused: Mutex<Option<Choice>>,
}

impl Component for ClosingPanel {
  type Props = PanelProps;

  fn create(ctx: &mut Ctx) -> Self {
    let tick = ctx.signal(0_u64);
    let next = tick.clone();
    let interval = ctx.create_interval(TICK, move || next.update(|tick| *tick += 1));
    ctx.props::<PanelProps>().focus.set(None);
    Self {
      tick,
      interval,
      buttons: [
        (Choice::QuitNow, ElementRef::new()),
        (Choice::Cancel, ElementRef::new()),
        (Choice::Wait, ElementRef::new()),
      ],
      focused: Mutex::new(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let PanelProps { phase, close, focus } = ctx.props::<PanelProps>().clone();
    let runners = Services::of(ctx).runners;
    work_changes(ctx, &runners);
    self.tick.get();
    self.interval.start();
    let since = match phase {
      ClosePhase::Confirming => None,
      ClosePhase::Waiting { since } => Some(since),
      ClosePhase::Open | ClosePhase::Stopping => return Element::new(),
    };
    let board = close.board();
    let stage = board.stage();
    self.move_focus(ctx, focus.get());
    // A start after now (the clock was set back since) has waited no time yet.
    let elapsed = since.map(|since| since.elapsed().unwrap_or_default());
    let mut panel = Column::new()
      .id("closing-panel")
      .describe("stage", stage_name(stage))
      .width(Dimension::Pct(100.0))
      .max_width(Dimension::Px(PANEL_WIDTH))
      .spacing(SpacingSize::Xl)
      .child(head(ctx, stage, elapsed))
      .with_children(groups(ctx, stage, &board, &runners));
    panel = match stage {
      Stage::Confirm => panel.with_children(waiting_note(ctx)).child(choices(ctx)),
      Stage::Waiting => panel.child(note("closing-too-long", &ctx.t("lifecycle.close.tooLong"))),
      Stage::Saved => panel,
    };
    let panel = panel.child(self.actions(ctx, stage, board.left(), &close));
    content(panel)
  }
}

impl ClosingPanel {
  /// Moves the focus to the button of `choice` when the keyboard chose another one.
  fn move_focus(&self, ctx: &Ctx, choice: Option<Choice>) {
    let mut focused = self.focused.lock();
    if *focused == choice {
      return;
    }
    *focused = choice;
    if let Some((_, button)) = self.buttons.iter().find(|(button, _)| Some(*button) == choice) {
      ctx.focus(button);
    }
  }

  fn button(&self, choice: Choice) -> ElementRef {
    self
      .buttons
      .iter()
      .find(|(button, _)| *button == choice)
      .map(|(_, element)| element.clone())
      .expect("every choice has its button")
  }
}

const fn stage_name(stage: Stage) -> &'static str {
  match stage {
    Stage::Confirm => "confirm",
    Stage::Waiting => "waiting",
    Stage::Saved => "saved",
  }
}

/// The panel in the content panel, where the page was, scrolling when it is taller than the window.
fn content(panel: Column) -> Element {
  let inner = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(LifecycleSpacing::PanelTop.role())
        .left(LifecycleSpacing::PanelSide.role())
        .right(LifecycleSpacing::PanelSide.role())
        .bottom(SpacingSize::Section),
    )
    .align_items(Alignment::Center)
    .child(panel);
  let panel = Column::new()
    .id("closing-content")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .background(PaletteColor::SurfacePanel)
    .rounded(RadiusSize::Lg)
    .border_inside(BorderSize::Sm, ExtraColor::PanelBorder.role())
    .clip()
    .child(
      ScrollVertical::new(inner)
        .id("closing-scroll")
        .width(Dimension::Pct(100.0))
        .height(Dimension::Pct(100.0)),
    );
  // `Content wrap` (`mhYqz`): the sidebar gives way; 8 between the panel and the window's sides.
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .background(PaletteColor::SurfaceBase)
    .padding(Padding::horizontal(SpacingSize::Sm))
    .child(panel)
    .into()
}

/// One group of runs per project, in the order of the runs (by project name), then git's operations by
/// project.
fn groups(ctx: &Ctx, stage: Stage, board: &Board, runners: &Runners) -> Vec<Element> {
  let mut projects: Vec<&str> = Vec::new();
  for (work, _) in &board.runs {
    if !projects.contains(&work.project()) {
      projects.push(work.project());
    }
  }
  let mut groups: Vec<Element> = projects
    .into_iter()
    .map(|project| {
      let runs: Vec<Shown<'_>> = board
        .runs
        .iter()
        .filter(|(work, _)| work.project() == project)
        .map(|(work, stands)| Shown {
          work,
          stands,
          last_event: runners.last_event(work.run()),
        })
        .collect();
      let title = group_title(ctx, stage, project, &runs);
      group(ctx, project, &title, &runs)
    })
    .collect();
  let mut git: Vec<&str> = Vec::new();
  for job in &board.jobs {
    if !git.contains(&&*job.project) {
      git.push(&job.project);
    }
  }
  groups.extend(git.into_iter().map(|project| {
    let jobs: Vec<&BusyJob> = board.jobs.iter().filter(|job| &*job.project == project).collect();
    jobs_group(ctx, project, &jobs)
  }));
  groups
}

/// *In progress · Kontur*, or *2 of 3 runs saved · Kontur*.
fn group_title(ctx: &Ctx, stage: Stage, project: &str, runs: &[Shown<'_>]) -> std::sync::Arc<str> {
  if stage == Stage::Confirm {
    return ctx.t_args("lifecycle.close.group.inProgress", [("project", project)]);
  }
  let key = if runs.len() == 1 {
    "lifecycle.close.group.saved.one"
  } else {
    "lifecycle.close.group.saved.other"
  };
  let saved = runs
    .iter()
    .filter(|run| matches!(run.stands, RunStands::Saved { .. }))
    .count()
    .to_string();
  let count = runs.len().to_string();
  ctx.t_args(
    key,
    [
      ("saved", saved.as_str()),
      ("count", count.as_str()),
      ("project", project),
    ],
  )
}

/// The runners' work revision; reading it renders the panel again whenever a runner reports.
fn work_changes(ctx: &mut Ctx, runners: &Runners) -> u64 {
  let changes = ctx.stream(runners.clone(), |runners, emitter: StreamEmitter<u64, ()>| async move {
    let mut updates = runners.work_changes();
    loop {
      let revision = *updates.borrow_and_update();
      if !emitter.emit(revision) || updates.changed().await.is_err() {
        return;
      }
    }
  });
  changes.state().get().data.unwrap_or(0)
}
