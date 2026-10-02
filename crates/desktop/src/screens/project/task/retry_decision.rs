//! Allow another attempt (`Lk43S` in 02.B `J3JZaT`) for a run that waits for a person after its
//! step's retries ran out or the step broke again: what the run used and has left, then the three
//! decisions the runner carries out ([`StuckDecision`]) — *Allow 1 retry & continue* (the same
//! route, or the broken step again), returning the work to a chosen agent step with a note for its
//! agent, and stopping the run, which fails it. A decision under way disables them all; a refused
//! one says why under them.

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
    field::{FieldNote, ValueTone, field, field_error, select_style, value_box},
    form::form,
    icon::Icon,
    message_text,
    property::property,
  },
};

/// Panel width (`Lk43S`) and the height of the step choice (a select field, `ZGwlv`).
const PANEL_WIDTH: f32 = 360.0;
const TARGET_HEIGHT: f32 = 40.0;

/// The waiting run as the panel needs it; compared to remount the panel when the run changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetryDecisionProps {
  pub run: RunId,
  pub stuck: Stuck,
  /// "1.1M / 1.4M tokens" and "300k tokens".
  pub used: Arc<str>,
  pub left: Arc<str>,
  /// The step one more retry starts from, and the step that failed, by name.
  pub retry_from: Arc<str>,
  pub failed: Arc<str>,
  /// The agent steps the work can return to, with their names, in pipeline order.
  pub targets: Arc<[(StepKey, Arc<str>)]>,
}

impl lurq::app::component::DevtoolsInspectable for RetryDecisionProps {}

/// The step the work returns to; `None` until one is chosen, which reads as the first.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Target(Option<StepKey>);

impl lurq::app::component::DevtoolsInspectable for Target {}

type DecisionAction = FutureAction<(RunId, StuckDecision), (), Message>;

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
    let decide = |decision: StuckDecision| {
      let (action, run) = (action.clone(), props.run);
      move || action.run((run, decision.clone()))
    };
    let target = self.chosen(&props);
    let note = ReviewNote::new(&self.note.get()).ok();
    let returning = target.clone().zip(note).filter(|_| idle);
    let target_name = target
      .as_ref()
      .and_then(|key| props.targets.iter().find(|(step, _)| step == key))
      .map_or_else(|| Arc::from(""), |(_, name)| name.clone());
    let failure = state.error.filter(|_| idle).map(|error| message_text(ctx, &error));
    let retry = ActionButton::new(ctx.t("runs.retryLimit.allow"))
      .icon(if idle { Icon::RefreshCw } else { Icon::Loader })
      .primary()
      .fill_width()
      .id("task-allow-retry")
      .in_form();
    let retry = if idle {
      retry.on_click(decide(StuckDecision::Retry))
    } else {
      retry.disabled()
    };
    let back = ActionButton::new(ctx.t_args("runs.retryLimit.returnWithNote", [("step", &*target_name)]))
      .icon(Icon::MessageSquare)
      .fill_width()
      .id("task-return-with-note")
      .in_form();
    let back = match returning {
      Some((step, note)) => back.on_click(decide(StuckDecision::ReturnTo { step, note })),
      None => back.disabled(),
    };
    let stop = ActionButton::new(ctx.t("runs.retryLimit.stop"))
      .icon(Icon::Square)
      .fill_width()
      .id("task-stop-run")
      .in_form()
      .enabled(idle);
    let stop = if idle {
      stop.on_click(decide(StuckDecision::Stop))
    } else {
      stop
    };
    let content = Column::new()
      .id("task-retry-settings")
      .width(Dimension::Px(PANEL_WIDTH))
      .height(Dimension::Pct(100.0))
      .padding(SpacingSize::Lg)
      .spacing(ExtraSpacing::Form.role())
      .background(PaletteColor::SurfacePanel)
      .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(Text::new(&ctx.t("runs.retryLimit.allowTitle")).variant(TypographyStyle::Title))
      .child(property(&ctx.t("runs.retryLimit.used"), &props.used))
      .child(property(&ctx.t("runs.retryLimit.remaining"), &props.left))
      .child(self.retry_fields(ctx, &props))
      .child(retry.render(ctx))
      .child(self.return_fields(ctx, &props))
      .child(back.render(ctx))
      .child(Spacer::new().flex(1.0))
      .with_children(failure.map(|error| field_error(&error)))
      .child(stop.render(ctx));
    form(None, content)
  }
}

impl RetryDecisionPanel {
  /// Chooses the first step the work can return to, once.
  fn seed(&self, props: &RetryDecisionProps) {
    if self.seeded.get_untracked() {
      return;
    }
    self.seeded.set(true);
    self
      .target
      .set(Target(props.targets.first().map(|(step, _)| step.clone())));
  }

  /// The chosen step, if it is one the work can return to.
  fn chosen(&self, props: &RetryDecisionProps) -> Option<StepKey> {
    self
      .target
      .get()
      .0
      .filter(|key| props.targets.iter().any(|(step, _)| step == key))
  }

  /// What one more retry takes and does (`QLlzp`, `bNYu5`, `zbgRK`).
  fn retry_fields(&self, ctx: &Ctx, props: &RetryDecisionProps) -> Element {
    let resume = ctx.t_args("runs.retryLimit.resumeValue", [("step", &*props.retry_from)]);
    let meaning = match props.stuck {
      Stuck::RetriesExhausted(_) => ctx.t_args(
        "runs.retryLimit.retryMeaning",
        [("step", &*props.retry_from), ("failed", &*props.failed)],
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
      .spacing(ExtraSpacing::Form.role())
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

  /// The step to return the work to and the note for its agent.
  fn return_fields(&self, ctx: &Ctx, props: &RetryDecisionProps) -> Element {
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
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Field.role())
      .child(field(&ctx.t("runs.retryLimit.returnTo"), select, FieldNote::None))
      .child(note_field(
        ctx,
        (&self.note, &self.note_focused),
        "task-return-note",
        &ctx.t("runs.retryLimit.notePlaceholder"),
      ))
      .into()
  }
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
