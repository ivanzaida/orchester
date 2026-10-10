//! The closing panel in the content panel, where the page was (`Closing panel` of `u3Fw36`, `lxm8W`,
//! `n5psM`): what closing does, the runs with an agent at work by project, the tasks that only wait for a
//! person, the two choices and the actions. While the agents save, it follows the runners' reports and
//! the time, and says how long it has waited; once every run is saved it says Orchester closes.

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Interval, StreamEmitter},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{
  close::{CloseFlow, ClosePhase, PersonWait, WaitingTask, waiting_for_person},
  runner::{ActiveWork, Runners},
};
use parking_lot::Mutex;

use super::{
  keys::Choice,
  rows::{Shown, Stands, group},
};
use crate::{
  navigation::Navigation,
  project::Projects,
  services::Services,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize, LifecycleSpacing, LifecycleText, StartupText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, glyph, icon},
  },
};

/// The closing panel's width (`Closing panel` `v9psPQ`).
const PANEL_WIDTH: f32 = 880.0;
/// The icon tile beside the heading (`Icon tile` `H4Wnl`).
const TILE_SIZE: f32 = 40.0;
/// How often the waiting panel says the time again.
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

/// What the panel says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
  /// The person decides (`u3Fw36`).
  Confirm,
  /// The agents save (`lxm8W`).
  Waiting,
  /// Every run is saved; the runners exit and Orchester closes (`n5psM`).
  Saved,
}

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
    let (work, since) = match phase {
      ClosePhase::Confirming(work) => (work, None),
      ClosePhase::Waiting { work, since } => (work, Some(since)),
      ClosePhase::Open | ClosePhase::Stopping => return Element::new(),
    };
    let stands: Vec<Stands> = work
      .iter()
      .map(|work| match since {
        None => Stands::Running,
        Some(_) => Stands::waiting(runners.save_state(work.run()), runners.last_event(work.run())),
      })
      .collect();
    let stage = match since {
      None => Stage::Confirm,
      Some(_) if stands.iter().all(Stands::is_saved) => Stage::Saved,
      Some(_) => Stage::Waiting,
    };
    if stage == Stage::Waiting {
      self.interval.start();
    } else {
      self.interval.stop();
    }
    self.move_focus(ctx, focus.get());
    let elapsed = since.and_then(|since| since.elapsed().ok());
    let mut panel = Column::new()
      .id("closing-panel")
      .describe("stage", stage_name(stage))
      .width(Dimension::Pct(100.0))
      .max_width(Dimension::Px(PANEL_WIDTH))
      .spacing(SpacingSize::Xl)
      .child(head(ctx, stage, elapsed))
      .with_children(groups(ctx, stage, &work, &stands));
    panel = match stage {
      Stage::Confirm => panel.with_children(waiting_note(ctx)).child(choices(ctx)),
      Stage::Waiting => panel.child(note("closing-too-long", &ctx.t("lifecycle.close.tooLong"))),
      Stage::Saved => panel,
    };
    let saving = stands.iter().filter(|stands| !stands.is_saved()).count();
    let panel = panel.child(self.actions(ctx, stage, saving, &close));
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

  /// Quit now on the left, Cancel and Wait for agents to save on the right (`Action bar` `giZVL`). While
  /// the agents save Cancel is unavailable, with why, and the primary becomes how many runs Orchester
  /// still waits for; once every run is saved only *Closing Orchester…* is left.
  fn actions(&self, ctx: &Ctx, stage: Stage, saving: usize, close: &CloseFlow) -> Element {
    let press = |choice: Choice| {
      let close = close.clone();
      move || choice.press(&close)
    };
    let mut row = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center);
    if stage != Stage::Saved {
      row = row.child(
        ActionButton::new(
          "closing-quit-now",
          ButtonKind::Secondary,
          ctx.t("lifecycle.close.quitNow"),
        )
        .icon(Icon::Power)
        .in_form()
        .element_ref(self.button(Choice::QuitNow))
        .on_click(press(Choice::QuitNow)),
      );
    }
    row = row.child(Spacer::new().flex(1.0));
    let cancel = ActionButton::new("closing-cancel", ButtonKind::Secondary, ctx.t("lifecycle.close.cancel"))
      .in_form()
      .element_ref(self.button(Choice::Cancel));
    let actions: Element = match stage {
      Stage::Confirm => row
        .child(cancel.on_click(press(Choice::Cancel)))
        .child(
          ActionButton::new("closing-wait", ButtonKind::Primary, ctx.t("lifecycle.close.wait"))
            .icon(Icon::Save)
            .in_form()
            .element_ref(self.button(Choice::Wait))
            .on_click(press(Choice::Wait)),
        )
        .into(),
      Stage::Waiting => {
        let count = saving.to_string();
        let key = if saving == 1 {
          "lifecycle.close.closesAfter.one"
        } else {
          "lifecycle.close.closesAfter.other"
        };
        let row = row
          .child(cancel.disabled())
          .child(progress(&ctx.t_args(key, [("count", count.as_str())])));
        // `Reason line` (`NGPaQ`): the lock and why, at the actions' end, 8 under them.
        let reason = Row::new()
          .width(Dimension::Pct(100.0))
          .justify(Justify::End)
          .spacing(ExtraSpacing::Snug.role())
          .align_items(Alignment::Center)
          .child(icon(Icon::Lock, IconSize::Sm, PaletteColor::TextMuted))
          .child(
            Text::new(&ctx.t("lifecycle.close.cancelUnavailable"))
              .id("closing-cancel-reason")
              .variant(ExtraTypography::Note.role())
              .nowrap(),
          );
        Column::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .child(row)
          .child(reason)
          .into()
      }
      Stage::Saved => row.child(progress(&ctx.t("lifecycle.close.closing"))).into(),
    };
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().top(SpacingSize::Lg))
      .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(actions)
      .into()
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

/// The icon tile, the heading and what it means; while the agents save, how long it has waited
/// (`Head` `d5mNQ`, `lxswN`, `b0VfYe`).
fn head(ctx: &Ctx, stage: Stage, elapsed: Option<Duration>) -> Element {
  let (glyph_icon, color, title, description) = match stage {
    Stage::Confirm => (
      Icon::Power,
      PaletteColor::TextSecondary,
      "lifecycle.close.confirm.title",
      "lifecycle.close.confirm.description",
    ),
    Stage::Waiting => (
      Icon::LoaderCircle,
      PaletteColor::TextSecondary,
      "lifecycle.close.waiting.title",
      "lifecycle.close.waiting.description",
    ),
    Stage::Saved => (
      Icon::CircleCheck,
      PaletteColor::Success,
      "lifecycle.close.saved.title",
      "lifecycle.close.saved.description",
    ),
  };
  let tile_glyph = Text::new(&glyph(glyph_icon))
    .variant(StartupText::TileIcon.role())
    .color(color);
  let tile_glyph = if glyph_icon == Icon::LoaderCircle {
    crate::ui::loader::turning(tile_glyph)
  } else {
    tile_glyph
  };
  let tile = Row::new()
    .width(Dimension::Px(TILE_SIZE))
    .height(Dimension::Px(TILE_SIZE))
    .flex_shrink(0.0)
    .rounded(RadiusSize::Lg)
    .background(PaletteColor::SurfaceRaised)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(tile_glyph);
  let elapsed = (stage == Stage::Waiting).then(|| elapsed_line(ctx, elapsed.unwrap_or_default()));
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .child(tile)
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Snug.role())
        .child(
          Text::new(&ctx.t(title))
            .id("closing-title")
            .variant(TypographyStyle::Heading),
        )
        .child(
          Text::new(&ctx.t(description))
            .variant(TypographyStyle::Description)
            .width(Dimension::Pct(100.0)),
        )
        .with_children(elapsed),
    )
    .into()
}

/// *Waiting for 1 min 12 s* (`Elapsed` `loGeE`), from when Wait for agents to save was chosen.
fn elapsed_line(ctx: &Ctx, elapsed: Duration) -> Element {
  let seconds = elapsed.as_secs();
  let words = if seconds < 60 {
    ctx.t_args("lifecycle.close.elapsed.seconds", [("seconds", seconds.to_string())])
  } else {
    ctx.t_args(
      "lifecycle.close.elapsed.minutes",
      [
        ("minutes", (seconds / 60).to_string()),
        ("seconds", (seconds % 60).to_string()),
      ],
    )
  };
  Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Timer, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&words)
        .id("closing-elapsed")
        .variant(ExtraTypography::Note.role()),
    )
    .into()
}

/// One group per project, in the order of the runs (by project name).
fn groups(ctx: &Ctx, stage: Stage, work: &[ActiveWork], stands: &[Stands]) -> Vec<Element> {
  let mut projects: Vec<&str> = Vec::new();
  for work in work {
    if !projects.contains(&work.project()) {
      projects.push(work.project());
    }
  }
  projects
    .into_iter()
    .map(|project| {
      let runs: Vec<Shown<'_>> = work
        .iter()
        .zip(stands)
        .filter(|(work, _)| work.project() == project)
        .map(|(work, stands)| Shown {
          work,
          stands: stands.clone(),
        })
        .collect();
      let title = group_title(ctx, stage, project, &runs);
      group(ctx, project, &title, &runs)
    })
    .collect()
}

/// *In progress · Kontur*, or *2 of 3 runs saved · Kontur*.
fn group_title(ctx: &Ctx, stage: Stage, project: &str, runs: &[Shown<'_>]) -> Arc<str> {
  if stage == Stage::Confirm {
    return ctx.t_args("lifecycle.close.group.inProgress", [("project", project)]);
  }
  let key = if runs.len() == 1 {
    "lifecycle.close.group.saved.one"
  } else {
    "lifecycle.close.group.saved.other"
  };
  let saved = runs.iter().filter(|run| run.stands.is_saved()).count().to_string();
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

/// The tasks of the projects the tabs show that only wait for a person, named once (`Note` `QeO8y`).
fn waiting_note(ctx: &mut Ctx) -> Option<Element> {
  let projects = Projects::of(ctx);
  let tabs = Navigation::of(ctx).tabs();
  let mut shown = Vec::new();
  let mut waiting: Vec<WaitingTask> = Vec::new();
  for project in tabs.tabs().iter().filter_map(|tab| tab.route().project()) {
    if shown.contains(&project)
      || projects
        .state(project)
        .and_then(|state| state.open().cloned())
        .is_none()
    {
      continue;
    }
    shown.push(project);
    let tasks = projects
      .feed(project)
      .tasks
      .with(|tasks| tasks.ready().map(waiting_for_person));
    waiting.extend(tasks.unwrap_or_default());
  }
  let words = match waiting.as_slice() {
    [] => return None,
    [one] => {
      let key = match one.wait {
        PersonWait::Plan => "lifecycle.close.waitingNote.plan",
        PersonWait::Answer => "lifecycle.close.waitingNote.answer",
        PersonWait::Decision => "lifecycle.close.waitingNote.decision",
      };
      ctx.t_args(key, [("task", &*one.key)])
    }
    several => {
      let separator = ctx.t("lifecycle.close.waitingNote.separator");
      let tasks: Vec<&str> = several.iter().map(|task| &*task.key).collect();
      ctx.t_args(
        "lifecycle.close.waitingNote.several",
        [("tasks", tasks.join(&separator))],
      )
    }
  };
  Some(note("closing-waiting-note", &words))
}

/// An information line under the rows (`Note` `QeO8y`, `gI4rP`).
fn note(id: &str, words: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(icon(Icon::Info, IconSize::Md, PaletteColor::TextMuted))
    .child(
      Text::new(words)
        .id(id)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .flex(1.0),
    )
    .into()
}

/// What each choice does (`Choices` `sK6LZ`).
fn choices(ctx: &Ctx) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Stretch)
    .child(choice(
      ctx,
      Icon::Save,
      "lifecycle.close.wait",
      "lifecycle.close.choice.waitDetail",
    ))
    .child(choice(
      ctx,
      Icon::Power,
      "lifecycle.close.quitNow",
      "lifecycle.close.choice.quitDetail",
    ))
    .into()
}

/// A choice's card (`Choice card` `zdSKi`).
fn choice(ctx: &Ctx, glyph_icon: Icon, title: &str, detail: &str) -> Column {
  Column::new()
    .flex(1.0)
    .padding(SpacingSize::Lg)
    .spacing(ExtraSpacing::Snug.role())
    .background(PaletteColor::SurfaceRaised)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .rounded(RadiusSize::Lg)
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(icon(glyph_icon, IconSize::Nav, PaletteColor::TextSecondary))
        .child(Text::new(&ctx.t(title)).variant(LifecycleText::ChoiceTitle.role())),
    )
    .child(
      Text::new(&ctx.t(detail))
        .variant(TypographyStyle::Description)
        .width(Dimension::Pct(100.0)),
    )
}

/// *Closes after 2 more runs*, *Closing Orchester…*: shaped like a button, but not one (`Progress pill`
/// `jRUlg`).
fn progress(words: &str) -> Element {
  Row::new()
    .id("closing-progress")
    .height(Dimension::Px(crate::theme::ControlHeight::Md.value()))
    .padding(Padding::horizontal(SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .rounded(RadiusSize::Md)
    .child(icon(Icon::LoaderCircle, IconSize::Md, PaletteColor::TextSecondary))
    .child(Text::new(words).variant(TypographyStyle::Button).nowrap())
    .into()
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
