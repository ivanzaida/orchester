//! The run's model chip in the composer's row (ORC-322; `Model chip` `VJUdj` in 08 `DPofN`, its states in 08f `Yzb3F`):
//! a 12 `cpu` and what the step's next turn runs on, *{model} · {effort}*, in 11.5 `text_secondary`, and a 12
//! `chevron-down` in `text_muted`; 6 apart, 8 at its sides, radius 6, the `pressed` fill while its picker is open and
//! `hover` under the pointer. While a change waits for the turn that runs, its glyph is an `hourglass` in `warning` and
//! *after this turn* follows in `text_muted`; on a connection with nothing to choose from, a `lock`. Its tooltip keeps
//! the profile's model and effort. It opens the step's picker ([`picker`]), 380 wide, its start at the chip's and its
//! bottom 8 over the composer's box.
//!
//! A change from the picker or a cancelation from an event line ([`RunModel`]) is the run's own, from the step's next
//! turn (`orchester_workspace::tasks::run_model::change`); the pipeline and the profile never change from it.

mod picker;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    events::is_synthetic_input,
    theme::{PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Placement, Popup, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, Style},
};
use orchester_core::{
  id::{ProjectId, RunId},
  resource::StepKey,
  run::{Answerer, ModelChoice, ModelScope},
};
use orchester_workspace::{
  background,
  messages::{self, Message},
  tasks::{
    detail::{RunView, TaskDetail},
    run_model::{
      StepModel,
      change::{ModelAsked, ModelChangeError, cancel_model_change_in, change_model_in},
    },
  },
};

use crate::{
  project::{ProjectFeed, ShownProject},
  screens::task::conversation::timeline::step_label,
  theme::{ExtraColor, ExtraSpacing, IconSize, ModelText},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
    tooltip::Tooltip,
  },
};

/// What is asked of a run's model: a change, or the cancelation of one that waits.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Asked {
  Change(ModelAsked),
  Cancel(StepKey),
}

/// A change or cancelation asked: where, which run, what, and who asked.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Asking {
  root: PathBuf,
  run: RunId,
  asked: Asked,
  by: Answerer,
}

impl DevtoolsInspectable for Asking {}

/// Why a change or cancelation was not made, as the picker says it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Failed {
  /// The connection does not offer the model or effort any more.
  NotOffered,
  /// The turn ended first: the change applies already.
  NotWaiting,
  /// The run ended, or its step's profile or connection is gone.
  Unavailable,
  Other(Message),
}

impl DevtoolsInspectable for Failed {}

/// The run's model as the tab's chip and its event lines change it: whether the picker is open, and the change under
/// way. Provided once by the tab.
#[derive(Clone)]
pub(super) struct RunModel {
  pub(super) open: Signal<bool>,
  change: FutureAction<Asking, (), Failed>,
}

impl RunModel {
  /// Provides the tab's run model; `feed` reads the task again once a change was made.
  pub(super) fn provide(ctx: &mut Ctx, feed: ProjectFeed) {
    let change = ctx.future_action(|asking: Asking| async move {
      let made = background::run(move || match asking.asked {
        Asked::Change(asked) => change_model_in(&asking.root, asking.run, asked, asking.by),
        Asked::Cancel(step) => cancel_model_change_in(&asking.root, asking.run, &step, asking.by),
      })
      .await
      .map_err(Failed::Other)?;
      made.map_err(|error| match error {
        ModelChangeError::NotOffered { .. } => Failed::NotOffered,
        ModelChangeError::NotWaiting => Failed::NotWaiting,
        ModelChangeError::Unavailable => Failed::Unavailable,
        ModelChangeError::Store(error) => {
          tracing::warn!(%error, "the run's model was not changed");
          Failed::Other(messages::store_error(&error))
        }
      })
    });
    ctx.watch(&change.state(), move |state| {
      if state.is_fulfilled() || state.is_rejected() {
        feed.saved();
      }
    });
    let model = Self {
      open: ctx.signal(false),
      change,
    };
    ctx.provide(model);
  }

  pub(super) fn of(ctx: &mut Ctx) -> Self {
    ctx
      .use_context::<Self>()
      .expect("the Conversation tab provides the run's model")
  }

  /// Asks `asked` of `run` of the project at `root`, as whoever gives the input now: a person, or an MCP client's
  /// synthetic input; nothing while a change is under way.
  fn ask(&self, root: &std::path::Path, run: RunId, asked: Asked) {
    if self.change.state().get_untracked().is_pending() {
      return;
    }
    let by = if is_synthetic_input() {
      Answerer::Automation
    } else {
      Answerer::Person
    };
    self.change.run(Asking {
      root: root.to_path_buf(),
      run,
      asked,
      by,
    });
  }

  /// Cancels the change of `step` of `run` that waits for its turn to end.
  pub(super) fn cancel(&self, root: &std::path::Path, run: RunId, step: StepKey) {
    self.ask(root, run, Asked::Cancel(step));
  }

  /// Why the last change was not made, while it shows.
  fn failure(&self) -> Option<Failed> {
    let state = self.change.state().get();
    let rejected = state.is_rejected();
    state.error.filter(|_| rejected)
  }
}

/// The chip of the step at work of the latest run of the task `detail` of `project` while the run goes on, in the
/// composer whose box is `box_anchor`.
pub(super) fn chip(
  ctx: &mut Ctx,
  (project, detail): (&ShownProject, &TaskDetail),
  box_anchor: &ElementRef,
) -> Option<Element> {
  let run = detail.run.as_ref()?;
  let step = run.models.step.clone()?;
  let props = ChipProps {
    root: project.open.repo_root.clone(),
    project: project.open.project.id(),
    run: run.id(),
    names: StepNames::of(ctx, run, &step),
    step,
    box_anchor: box_anchor.clone(),
  };
  Some(ctx.mount_keyed::<ModelChip>(&format!("conversation-model-{}", run.id()), props))
}

/// The step at work and the later steps a change reaches or not, as *03 Implement* names them.
#[derive(Clone, Debug, PartialEq, Eq)]
struct StepNames {
  step: Arc<str>,
  changing: Vec<Arc<str>>,
  keeping: Vec<Arc<str>>,
}

impl StepNames {
  fn of(ctx: &Ctx, run: &RunView, step: &StepModel) -> Self {
    let label = |key: &StepKey| step_name(ctx, run, key);
    Self {
      step: label(&step.step),
      changing: step.later.changing.iter().map(label).collect(),
      keeping: step.later.keeping.iter().map(label).collect(),
    }
  }
}

/// Step `key` of `run` as *03 Implement*; its key once the run's pipeline no longer has it.
pub(super) fn step_name(ctx: &Ctx, run: &RunView, key: &StepKey) -> Arc<str> {
  run
    .steps
    .iter()
    .find(|step| step.key == *key)
    .map_or_else(|| key.as_str().into(), |step| step_label(ctx, step.number, &step.name))
}

/// What the chip and its picker show of one run's step at work.
#[derive(Clone)]
struct ChipProps {
  root: PathBuf,
  project: ProjectId,
  run: RunId,
  step: StepModel,
  names: StepNames,
  /// The composer's box, which the picker opens over.
  box_anchor: ElementRef,
}

/// Compared by what it shows; the box is the composer's for as long as it shows.
impl PartialEq for ChipProps {
  fn eq(&self, other: &Self) -> bool {
    self.root == other.root
      && self.project == other.project
      && self.run == other.run
      && self.step == other.step
      && self.names == other.names
  }
}

impl DevtoolsInspectable for ChipProps {}

/// The model and effort chosen in the picker; `None` until one is, while it shows what the step runs on now.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Chosen(Option<ModelChoice>);

impl DevtoolsInspectable for Chosen {}

/// What the picker holds until *Apply*: the model and effort chosen, whether they hold for the rest of the run, and
/// whether the effort's options show.
#[derive(Clone)]
struct Draft {
  chosen: Signal<Chosen>,
  for_run: Signal<bool>,
  efforts: Signal<bool>,
}

impl Draft {
  fn chosen(&self) -> Option<ModelChoice> {
    self.chosen.get().0
  }

  fn scope(&self) -> ModelScope {
    if self.for_run.get() {
      ModelScope::Run
    } else {
      ModelScope::Step
    }
  }

  /// Forgets what was chosen: the picker opens again on what the step runs on now.
  fn forget(&self) {
    self.chosen.set(Chosen::default());
    self.for_run.set(false);
    self.efforts.set(false);
  }
}

struct ModelChip {
  anchor: ElementRef,
  draft: Draft,
}

impl Component for ModelChip {
  type Props = ChipProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      anchor: ElementRef::new(),
      draft: Draft {
        chosen: ctx.signal(Chosen::default()),
        for_run: ctx.signal(false),
        efforts: ctx.signal(false),
      },
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ChipProps>().clone();
    let model = RunModel::of(ctx);
    let open = model.open.get();
    if !open && (self.draft.chosen.get_untracked().0.is_some() || self.draft.for_run.get_untracked()) {
      self.draft.forget();
    }
    let toggle = model.open.clone();
    let chip = pressable(move || toggle.update(|open| *open = !*open))
      .id("conversation-model")
      .ref_element(self.anchor.clone())
      .describe("state", chip_state(&props.step))
      .describe("open", if open { "true" } else { "false" })
      .describe(
        "a11y_name",
        ctx.t_args("conversation.model.chip.name", [("step", &*props.names.step)]),
      )
      .padding_horizontal(SpacingSize::Sm)
      .rounded(RadiusSize::Md)
      .hovered_style(Style::new().background(ExtraColor::Hover.role()));
    let chip = if open {
      chip.background(ExtraColor::Pressed.role())
    } else {
      chip
    }
    .child(face(ctx, &props.step));
    let tip = ctx.t_args(
      "conversation.model.chip.tip",
      [("choice", &*choice_words(ctx, &props.step.profile))],
    );
    let chip = Tooltip::new("conversation-model", tip).wrap(ctx, chip);
    let panel = picker::panel(ctx, &props, (&model, &self.draft));
    // The picker's start at the chip's, its bottom 8 over the composer's box.
    let start = self.anchor.x() - props.box_anchor.x();
    let gap = ctx.theme().spacing_value(SpacingSize::Sm).to_px();
    let popup = Popup::new(props.box_anchor.clone(), panel)
      .open(model.open.clone())
      .switch_on_anchor_press(false)
      .placement(Placement::TopStart)
      .offset(start, gap);
    Row::new().child(chip).child(popup)
  }
}

/// The chip's glyph, what the step's next turn runs on, *after this turn* while a change waits, and its chevron.
fn face(ctx: &Ctx, step: &StepModel) -> Row {
  let (glyph, color) = match (step.waiting.is_some(), step.fixed()) {
    (true, _) => (Icon::Hourglass, PaletteColor::Warning),
    (false, true) => (Icon::Lock, PaletteColor::TextSecondary),
    (false, false) => (Icon::Cpu, PaletteColor::TextSecondary),
  };
  let after = step.waiting.is_some().then(|| {
    Text::new(&ctx.t("conversation.model.chip.afterTurn"))
      .id("conversation-model-after")
      .variant(ModelText::Chip.role())
      .color(PaletteColor::TextMuted)
      .nowrap()
  });
  Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Sm, color).id("conversation-model-icon"))
    .child(
      Text::new(&choice_words(ctx, &step.now))
        .id("conversation-model-label")
        .variant(ModelText::Chip.role())
        .nowrap(),
    )
    .with_children(after)
    .child(icon(Icon::ChevronDown, IconSize::Sm, PaletteColor::TextMuted))
}

/// A model and its effort as the chip says them, *claude-opus-5-5 · Medium*; the model alone without an effort.
pub(super) fn choice_words(ctx: &Ctx, choice: &ModelChoice) -> Arc<str> {
  match &choice.effort {
    Some(effort) => ctx.t_args(
      "conversation.model.choice",
      [("model", choice.model.as_str()), ("effort", &*effort_word(ctx, effort))],
    ),
    None => choice.model.as_str().into(),
  }
}

/// The word for reasoning effort `effort`; an effort with no word of its own shows as the CLI names it.
pub(super) fn effort_word(ctx: &Ctx, effort: &str) -> Arc<str> {
  match effort {
    "minimal" => ctx.t("conversation.model.efforts.minimal"),
    "low" => ctx.t("conversation.model.efforts.low"),
    "medium" => ctx.t("conversation.model.efforts.medium"),
    "high" => ctx.t("conversation.model.efforts.high"),
    "xhigh" => ctx.t("conversation.model.efforts.xhigh"),
    "max" => ctx.t("conversation.model.efforts.max"),
    other => other.into(),
  }
}

/// The name `describe` reports the chip's state by, for tests and QA.
fn chip_state(step: &StepModel) -> &'static str {
  match (step.waiting.is_some(), step.fixed(), step.changed()) {
    (true, ..) => "waits",
    (false, true, _) => "fixed",
    (false, false, true) => "changed",
    (false, false, false) => "profile",
  }
}
