//! Allow another attempt (`Bi6pU` in 02.B3 `FjCWv`, 02.B4 `K7hbj`, 02.B5 `m3pDW`) for a run that
//! waits for a person after its step's retries ran out or the step broke again: what the run used
//! and has left, then the three decisions the runner carries out ([`StuckDecision`]) — *Allow 1
//! retry & continue* (the same route, or the broken step again), returning the work to a chosen
//! agent step with a note for its agent, and stopping the run, which fails it. The step to return to
//! starts as the one a retry resumes. A decision under way disables them all; the runner's refusal
//! shows above *Stop and fail the run* (02.B7 `s9ZQmK`). Without an agent step to return to, that
//! part is left out.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Select, Spacer, Text},
  core::Signal,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  id::RunId,
  resource::StepKey,
  run::{ReviewNote, Stuck, StuckDecision},
};
use orchester_ipc::Command;

use super::plan_review::note_field;
use crate::{
  messages::Message,
  runner::Runners,
  tasks::run::control_message,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::{ValueTone, select_style, value_box, wrapping_field_error},
    form::form,
    icon::Icon,
    message_text,
    property::property,
  },
};

/// Panel width (`Bi6pU`), the height of the step choice (a select field, `ZGwlv`) and of the note
/// for the agent (`L6QNtg`).
const PANEL_WIDTH: f32 = 360.0;
const TARGET_HEIGHT: f32 = 40.0;
const NOTE_HEIGHT: f32 = 76.0;

/// The waiting run as the panel needs it; compared to remount the panel when the run changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetryDecisionProps {
  pub run: RunId,
  pub stuck: Stuck,
  /// "1.1M / 1.4M tokens" and "300k tokens".
  pub used: Arc<str>,
  pub left: Arc<str>,
  /// The step one more retry starts from, its name, and the name of the step that failed.
  pub retry_from: StepKey,
  pub retry_from_name: Arc<str>,
  pub failed: Arc<str>,
  /// The agent steps the work can return to, with their names, in pipeline order.
  pub targets: Arc<[(StepKey, Arc<str>)]>,
}

impl lurq::app::component::DevtoolsInspectable for RetryDecisionProps {}

impl RetryDecisionProps {
  /// The step the choice starts with: the one a retry resumes, else the first there is.
  fn default_target(&self) -> Option<StepKey> {
    self
      .targets
      .iter()
      .find(|(step, _)| *step == self.retry_from)
      .or_else(|| self.targets.first())
      .map(|(step, _)| step.clone())
  }
}

/// The step the work returns to; `None` only before the panel is seeded.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Target(Option<StepKey>);

impl lurq::app::component::DevtoolsInspectable for Target {}

type DecisionAction = FutureAction<(RunId, StuckDecision), (), Message>;
type Press = Arc<dyn Fn() + Send + Sync>;

pub struct RetryDecisionPanel {
  note: Signal<String>,
  note_focused: Signal<bool>,
  target: Signal<Target>,
  seeded: Signal<bool>,
}

impl Component for RetryDecisionPanel {
  type Props = RetryDecisionProps;

  fn create(ctx: &mut Ctx) -> Self {
    decision_action(ctx);
    Self {
      note: ctx.signal(String::new()),
      note_focused: ctx.signal(false),
      target: ctx.signal(Target(None)),
      seeded: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RetryDecisionProps>().clone();
    self.seed(&props);
    let action = decision_action(ctx);
    let state = action.state().get();
    let idle = !state.is_pending();
    let decide = |decision: StuckDecision| -> Option<Press> {
      let (action, run) = (action.clone(), props.run);
      idle.then(|| Arc::new(move || action.run((run, decision.clone()))) as Press)
    };
    let failure = state.error.filter(|_| idle).map(|error| message_text(ctx, &error));
    let returning = (!props.targets.is_empty()).then(|| self.returning(ctx, &props, &decide));
    let content = Column::new()
      .id("task-retry-settings")
      .width(Dimension::Px(PANEL_WIDTH))
      .height(Dimension::Pct(100.0))
      .padding(SpacingSize::Lg)
      .spacing(ExtraSpacing::Form.role())
      .background(PaletteColor::SurfacePanel)
      .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(Text::new(&ctx.t("runs.retryLimit.allowTitle")).variant(TypographyStyle::Title))
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .child(property(&ctx.t("runs.retryLimit.used"), &props.used))
          .child(property(&ctx.t("runs.retryLimit.remaining"), &props.left)),
      )
      .child(retry_fields(ctx, &props))
      .child(
        button(
          (&ctx.t("runs.retryLimit.allow"), "task-allow-retry"),
          Icon::RefreshCw,
          decide(StuckDecision::Retry),
        )
        .primary()
        .render(ctx),
      )
      .with_children(returning.into_iter().flatten())
      .child(Spacer::new().flex(1.0))
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .spacing(ExtraSpacing::Stack.role())
          .with_children(failure.map(|error| wrapping_field_error(&error)))
          .child(
            button(
              (&ctx.t("runs.retryLimit.stop"), "task-stop-run"),
              Icon::Square,
              decide(StuckDecision::Stop),
            )
            .render(ctx),
          ),
      );
    form(None, content)
  }
}

impl RetryDecisionPanel {
  /// Starts the choice at the step a retry resumes, once.
  fn seed(&self, props: &RetryDecisionProps) {
    if self.seeded.get_untracked() {
      return;
    }
    self.seeded.set(true);
    self.target.set(Target(props.default_target()));
  }

  /// The step to return the work to, the note for its agent (`cSrkA`) and *Return to … with note*,
  /// which needs a note.
  fn returning(
    &self,
    ctx: &Ctx,
    props: &RetryDecisionProps,
    decide: &dyn Fn(StuckDecision) -> Option<Press>,
  ) -> [Element; 2] {
    let target = self
      .target
      .get()
      .0
      .filter(|key| props.targets.iter().any(|(step, _)| step == key));
    let name = target
      .as_ref()
      .and_then(|key| props.targets.iter().find(|(step, _)| step == key))
      .map_or_else(|| Arc::from(""), |(_, name)| name.clone());
    let note = ReviewNote::new(&self.note.get()).ok();
    let press = target
      .zip(note)
      .and_then(|(step, note)| decide(StuckDecision::ReturnTo { step, note }));
    let options = props
      .targets
      .iter()
      .map(|(step, name)| (Target(Some(step.clone())), name.clone()));
    let select = Element::from(
      Select::new(self.target.clone())
        .options(options)
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(TARGET_HEIGHT))
        .style(select_style(ctx, TARGET_HEIGHT)),
    )
    .id("task-return-step");
    let fields = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Field.role())
      .child(Text::new(&ctx.t("runs.retryLimit.returnTo")).variant(TypographyStyle::Caption))
      .child(select)
      .child(note_field(
        ctx,
        (&self.note, &self.note_focused),
        ("task-return-note", NOTE_HEIGHT),
        &ctx.t("runs.retryLimit.notePlaceholder"),
      ));
    let label = ctx.t_args("runs.retryLimit.returnWithNote", [("step", &*name)]);
    [
      fields.into(),
      button((&label, "task-return-with-note"), Icon::MessageSquare, press).render(ctx),
    ]
  }
}

/// A full-width decision button that takes `press`, or is unavailable without one.
fn button((label, id): (&str, &'static str), glyph: Icon, press: Option<Press>) -> ActionButton {
  let button = ActionButton::new(Arc::from(label))
    .icon(glyph)
    .fill_width()
    .id(id)
    .in_form();
  match press {
    Some(press) => button.on_click(move || press()),
    None => button.disabled(),
  }
}

/// What one more retry takes and does (`h5zEPw`): the attempts, where it resumes, and what follows.
fn retry_fields(ctx: &Ctx, props: &RetryDecisionProps) -> Element {
  let resume = ctx.t_args("runs.retryLimit.resumeValue", [("step", &*props.retry_from_name)]);
  let meaning = match props.stuck {
    Stuck::RetriesExhausted(_) => ctx.t_args(
      "runs.retryLimit.retryMeaning",
      [("step", &*props.retry_from_name), ("failed", &*props.failed)],
    ),
    Stuck::BrokeAgain(_) => ctx.t_args("runs.retryLimit.restartMeaning", [("step", &*props.failed)]),
  };
  let value = |label: &str, id: &'static str, value: &str| {
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Field.role())
      .child(Text::new(label).variant(TypographyStyle::Caption))
      .child(value_box(ctx, id, value, ValueTone::Value, None))
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(value(
      &ctx.t("runs.retryLimit.additional"),
      "task-retry-attempts",
      &ctx.t("runs.retryLimit.additionalValue"),
    ))
    .child(value(
      &ctx.t("runs.retryLimit.resumeFrom"),
      "task-retry-resume",
      &resume,
    ))
    .child(
      Text::new(&meaning)
        .width(Dimension::Pct(100.0))
        .variant(ExtraTypography::Explanation.role()),
    )
    .into()
}

fn decision_action(ctx: &mut Ctx) -> DecisionAction {
  let runners = ctx
    .use_context::<Runners>()
    .expect("the root provides Runners before mounting screens");
  ctx.future_action(move |(run, decision): (RunId, StuckDecision)| {
    let runners = runners.clone();
    async move {
      runners
        .control_run(Command::DecideStuckRun { run, decision })
        .await
        .map_err(|error| {
          tracing::warn!(%error, "the runner did not take the decision on the waiting run");
          control_message(&error)
        })
    }
  })
}
