//! A task's Artifacts tab (`Task · Artifacts · dark` `Mfru6` in `g5dmWI`), as its empty state draws it (`Empty ·
//! nothing saved yet` `IUHMS`): the toolbar with the run and how many artifacts it kept, and what artifacts are.
//! Runs keep no artifacts yet: no table holds them, no tool saves one and no tool step names its output files (the
//! design marks the tab a proposal, `D7liKl`). So every run reads *0 artifacts*, and the tab says plainly that runs
//! do not keep artifacts yet; the run picker chooses among the task's runs all the same.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::Justify, text_style::TextAlign},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::RunId, run::RunNumber};

use super::TaskContext;
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize, MemoryText, SecretsSpacing},
  ui::{
    filter::filter_menu_panel,
    icon::{Icon, icon},
    menu::{MenuItem, menu},
    picker::Picker,
  },
};

/// The toolbar's height (`P48TVr`), the run menu's width and the widest the explanation runs (`slhrm`).
const TOOLBAR_HEIGHT: f32 = 46.0;
const MENU_WIDTH: f32 = 220.0;
const EXPLANATION_WIDTH: f32 = 520.0;
/// The empty state's glyph (`package-open`, 22 in the design; the nearest glyph size).
const GLYPH_SIZE: IconSize = IconSize::Xl;

#[derive(Clone, PartialEq)]
pub struct ArtifactsProps {
  pub task: TaskContext,
}

impl DevtoolsInspectable for ArtifactsProps {}

pub fn tab(ctx: &mut Ctx, task: &TaskContext) -> Element {
  ctx.mount_keyed::<TaskArtifacts>("task-artifacts", ArtifactsProps { task: task.clone() })
}

/// The run the person chose; the latest until one is chosen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ChosenRun(Option<RunId>);

impl DevtoolsInspectable for ChosenRun {}

pub struct TaskArtifacts {
  /// The run picker's menu and the picker it opens at.
  menu: (Signal<bool>, ElementRef),
  chosen: Signal<ChosenRun>,
}

impl Component for TaskArtifacts {
  type Props = ArtifactsProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      menu: (ctx.signal(false), ElementRef::default()),
      chosen: ctx.signal(ChosenRun::default()),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ArtifactsProps>().clone();
    let runs = runs(&props.task);
    let latest = runs.first().map(|(id, _)| *id);
    let shown = self
      .chosen
      .get()
      .0
      .filter(|chosen| runs.iter().any(|(id, _)| id == chosen))
      .or(latest);
    let number = shown.and_then(|shown| runs.iter().find(|(id, _)| *id == shown).map(|(_, number)| *number));
    Column::new()
      .id("task-artifacts")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0))
      .child(self.toolbar(ctx, &runs, (shown, number), latest))
      .child(empty(ctx, number))
  }
}

/// The task's runs that can be read, latest first, with their numbers.
fn runs(task: &TaskContext) -> Vec<(RunId, RunNumber)> {
  let mut runs: Vec<(RunId, RunNumber)> = task
    .detail
    .history
    .iter()
    .filter_map(|record| record.number.map(|number| (record.id, number)))
    .collect();
  if let Some(latest) = &task.detail.run
    && !runs.iter().any(|(id, _)| *id == latest.id())
  {
    runs.push((latest.id(), latest.run.number()));
  }
  runs.sort_by_key(|(_, number)| std::cmp::Reverse(number.get()));
  runs
}

impl TaskArtifacts {
  /// *Run* with the picker of the task's runs, and the count (`P48TVr`).
  fn toolbar(
    &self,
    ctx: &mut Ctx,
    runs: &[(RunId, RunNumber)],
    (shown, number): (Option<RunId>, Option<RunNumber>),
    latest: Option<RunId>,
  ) -> Element {
    let mut bar = Row::new()
      .id("artifacts-toolbar")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(TOOLBAR_HEIGHT))
      .flex_shrink(0.0)
      .padding(Padding::horizontal(SpacingSize::Section))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border));
    if let (Some(shown), Some(number)) = (shown, number) {
      let label = run_label(ctx, number, Some(shown) == latest);
      bar = bar
        .child(
          Text::new(&ctx.t("artifacts.run"))
            .variant(TypographyStyle::Description)
            .color(PaletteColor::TextMuted),
        )
        .child(Picker::new(
          "artifacts-run",
          (Icon::History, label),
          self.menu.0.clone(),
          self.menu.1.clone(),
        ));
    }
    bar = bar
      .child(
        Text::new(&ctx.t("artifacts.none"))
          .id("artifacts-count")
          .variant(ExtraTypography::Note.role())
          .nowrap(),
      )
      .child(Spacer::new().flex(1.0));
    let panel = if self.menu.0.get() {
      self.run_menu(ctx, runs, (shown, latest))
    } else {
      Element::new()
    };
    bar.child(menu(self.menu.1.clone(), self.menu.0.clone(), panel)).into()
  }

  /// The task's runs to choose from, the shown one checked.
  fn run_menu(
    &self,
    ctx: &mut Ctx,
    runs: &[(RunId, RunNumber)],
    (shown, latest): (Option<RunId>, Option<RunId>),
  ) -> Element {
    let open = self.menu.0.clone();
    let mut panel = filter_menu_panel("artifacts-run-menu", MENU_WIDTH);
    for (id, number) in runs {
      let (chosen, run) = (self.chosen.clone(), *id);
      panel = panel.child(
        MenuItem::new(
          format!("artifacts-run-{}", number.get()),
          run_label(ctx, *number, Some(*id) == latest),
          move || chosen.set(ChosenRun(Some(run))),
        )
        .chosen(Some(*id) == shown)
        .render(ctx, &open),
      );
    }
    panel.into()
  }
}

/// *Run 128 · latest*, or *Run 120*.
fn run_label(ctx: &Ctx, number: RunNumber, latest: bool) -> std::sync::Arc<str> {
  let run = number.get().to_string();
  if latest {
    ctx.t_args("artifacts.latestRun", [("run", run)])
  } else {
    ctx.t_args("artifacts.runNumber", [("run", run)])
  }
}

/// *No artifacts in Run 128* and why: runs keep no artifacts yet (`kjUnI`).
fn empty(ctx: &Ctx, number: Option<RunNumber>) -> Element {
  let title = match number {
    Some(number) => ctx.t_args("artifacts.emptyRun", [("run", number.get().to_string())]),
    None => ctx.t("artifacts.empty"),
  };
  Column::new()
    .id("artifacts-empty")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .padding(Padding::horizontal(SecretsSpacing::Page.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(icon(Icon::PackageOpen, GLYPH_SIZE, PaletteColor::TextMuted))
    .child(
      Text::new(&title)
        .id("artifacts-empty-title")
        .variant(MemoryText::PanelTitle.role()),
    )
    .child(
      Text::new(&ctx.t("artifacts.emptyBody"))
        .id("artifacts-empty-body")
        .max_width(Dimension::Px(EXPLANATION_WIDTH))
        .variant(TypographyStyle::Description)
        .text_align(TextAlign::Center),
    )
    .into()
}
