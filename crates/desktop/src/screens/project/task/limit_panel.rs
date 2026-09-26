//! Raise the limit to continue (02.L `ZUYM0`): each limit with what the run used and an input
//! for the new limit with its read-back, what continuing does, and the one action that says the
//! result ("Continue with 2M tokens"). The input of the limit that was hit is focused. The new
//! limits apply to this run only; the budget is at most the project's maximum run budget.
//!
//! After a step's budget the panel raises that step's budget instead (*Raise the step budget to
//! continue*): what the stopped execution used of it and one token input. The published pipeline
//! keeps its budget; the raised one is at most the project's maximum run budget too.

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  id::RunId,
  limits::ProjectLimits,
  run::{RaisedLimits, RunLimit},
  time::Timestamp,
  tokens::{BudgetInputError, TokenBudget, TokensUsed, compact_tokens, grouped, parse_budget},
};
use orchester_ipc::Command;

use crate::{
  budget::{budget_error, minutes_text, read_budget, read_minutes, whole_minutes},
  messages::Message,
  runner::Runners,
  tasks::{detail::RunView, run::control_message},
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::{field_error, wrapping_field_error},
    form::{Submit, form},
    icon::Icon,
    message_text,
    unit_input::{FocusedInput, UnitInput},
  },
};

/// Width of a limit's input and read-back (`VolK1`), and the height of its input (`D0PKu`).
const INPUT_WIDTH: f32 = 220.0;
const INPUT_HEIGHT: f32 = 36.0;
/// A suggested budget is the current one and half again, rounded up to this.
const SUGGESTION_STEP: u64 = 100_000;
const MINUTE: Duration = Duration::from_secs(60);

/// The stopped run as the panel needs it; compared to remount the panel when the run changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RaiseLimitProps {
  run: RunId,
  number: u32,
  limit: RunLimit,
  /// Tokens used of, and the budget of, the run; after a step's budget, of that step's execution.
  used: TokensUsed,
  budget: TokenBudget,
  max_duration: Duration,
  active: Duration,
  max_budget: TokenBudget,
  /// The step and agent stopped mid-step, if the run stopped in a step.
  stopped: Option<(Arc<str>, Arc<str>)>,
  checkpoint: Option<String>,
}

impl lurq::app::component::DevtoolsInspectable for RaiseLimitProps {}

impl RaiseLimitProps {
  /// `None` for a stop at a step's budget when the step has none, which no runner records.
  pub fn of(run: &RunView, limit: RunLimit, limits: ProjectLimits) -> Option<Self> {
    let (used, budget) = match limit {
      RunLimit::StepBudget { execution } => run.step_budget_use(execution)?,
      RunLimit::Tokens | RunLimit::Time | RunLimit::DailyBudget => (run.run.used(), run.run.budget()),
    };
    Some(Self {
      run: run.id(),
      number: run.run.number().get(),
      limit,
      used,
      budget,
      max_duration: run.run.max_duration(),
      active: run.active_time(Timestamp::now()),
      max_budget: limits.max_run_budget(),
      stopped: run.stopped_step().map(|step| {
        let agent = step.agent.clone().unwrap_or_else(|| step.name.clone());
        (step.name.clone(), agent)
      }),
      checkpoint: run
        .run
        .last_checkpoint()
        .map(|checkpoint| crate::display::short_revision(checkpoint.revision().as_str()).to_owned()),
    })
  }

  const fn raises_step_budget(&self) -> bool {
    matches!(self.limit, RunLimit::StepBudget { .. })
  }

  /// The budget offered to continue with: after a token stop half again the current one, within
  /// the maximum; else unchanged.
  fn suggested_budget(&self) -> u64 {
    if self.limit == RunLimit::Time {
      return self.budget.get();
    }
    let raised = self.budget.get().saturating_add(self.budget.get() / 2);
    let rounded = raised.div_ceil(SUGGESTION_STEP).saturating_mul(SUGGESTION_STEP);
    rounded.min(self.max_budget.get()).max(self.budget.get())
  }

  /// The time limit offered, in whole minutes above the active time: after a time stop half
  /// again the current one, else unchanged.
  fn suggested_time(&self) -> Duration {
    let limit = match self.limit {
      RunLimit::Time => self.max_duration + self.max_duration / 2,
      RunLimit::Tokens | RunLimit::DailyBudget | RunLimit::StepBudget { .. } => self.max_duration,
    };
    let minutes = limit
      .as_secs()
      .div_ceil(MINUTE.as_secs())
      .max(whole_minutes(self.active) + 1);
    MINUTE * u32::try_from(minutes).unwrap_or(u32::MAX)
  }
}

/// What continuing raises: the run's own limits, or the budget of the step it stopped at.
#[derive(Clone, Copy, Debug)]
enum Raise {
  Limits(RaisedLimits),
  StepBudget(TokenBudget),
}

type RaiseAction = FutureAction<(RunId, Raise), (), Message>;

pub struct RaiseLimitPanel {
  budget: Signal<String>,
  minutes: Signal<String>,
  seeded: Signal<bool>,
  /// Which input has the focus, for its focused stroke.
  focused: FocusedInput,
  budget_input: ElementRef,
  time_input: ElementRef,
}

impl Component for RaiseLimitPanel {
  type Props = RaiseLimitProps;

  fn create(ctx: &mut Ctx) -> Self {
    raise_action(ctx);
    Self {
      budget: ctx.signal(String::new()),
      minutes: ctx.signal(String::new()),
      seeded: ctx.signal(false),
      focused: ctx.signal(None),
      budget_input: ElementRef::new(),
      time_input: ElementRef::new(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RaiseLimitProps>().clone();
    let action = raise_action(ctx);
    let separator = ctx.t("number.groupSeparator");
    self.seed(ctx, &props, &separator);
    let budget = self.read_budget(&props, &separator);
    let time = self.read_time(&props);
    let state = action.state().get();
    let pending = state.is_pending();
    let raise = match (&budget, &time) {
      (Ok(budget), _) if props.raises_step_budget() => Some(Raise::StepBudget(*budget)),
      (Ok(budget), Ok(max_duration)) => Some(Raise::Limits(RaisedLimits {
        budget: *budget,
        max_duration: *max_duration,
      })),
      _ => None,
    };
    let run = props.run;
    let submit: Submit = raise
      .filter(|_| !pending)
      .map(|raise| Arc::new(move || action.run((run, raise))) as Arc<dyn Fn() + Send + Sync>);
    let failure = state.error.filter(|_| !pending).map(|error| message_text(ctx, &error));
    let title = if props.raises_step_budget() {
      "runs.limit.raiseStepTitle"
    } else {
      "runs.limit.raiseTitle"
    };
    let content = Column::new()
      .id("raise-limit")
      .width(Dimension::Pct(100.0))
      .padding(ExtraSpacing::Step.role())
      .spacing(SpacingSize::Md)
      .background(PaletteColor::SurfacePanel)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .child(Text::new(&ctx.t(title)).variant(ExtraTypography::CardTitle.role()))
      .child(self.budget_row(ctx, &props, &budget, &separator))
      .with_children((!props.raises_step_budget()).then(|| self.time_row(ctx, &props, &time)))
      .child(
        Text::new(&explanation(ctx, &props))
          .variant(ExtraTypography::Detail.role())
          .width(Dimension::Pct(100.0)),
      )
      .with_children(failure.map(|error| field_error(&error)))
      .child(continue_button(
        ctx,
        &props,
        (budget.as_ref().ok(), time.as_ref().ok()),
        (pending, submit.clone()),
      ));
    form(submit, content)
  }
}

impl RaiseLimitPanel {
  /// Offers new limits once, and focuses the input of the limit that was hit.
  fn seed(&self, ctx: &Ctx, props: &RaiseLimitProps, separator: &str) {
    if self.seeded.get_untracked() {
      return;
    }
    self.seeded.set(true);
    self.budget.set(grouped(props.suggested_budget(), separator));
    self.minutes.set(minutes_text(props.suggested_time()));
    ctx.focus(match props.limit {
      RunLimit::Tokens | RunLimit::DailyBudget | RunLimit::StepBudget { .. } => &self.budget_input,
      RunLimit::Time => &self.time_input,
    });
  }

  fn read_budget(&self, props: &RaiseLimitProps, separator: &str) -> Result<TokenBudget, Message> {
    let used = compact_tokens(props.used.get());
    if !props.raises_step_budget() {
      let budget = read_budget(&self.budget.get(), separator, props.max_budget)?;
      if props.used.reached(budget) {
        return Err(Message::new("tokens.errors.notAboveUsed").with("used", used));
      }
      return Ok(budget);
    }
    let budget = parse_budget(&self.budget.get(), separator, props.max_budget).map_err(|error| match error {
      BudgetInputError::AboveMaximum { max } => {
        Message::new("tokens.errors.stepAboveMax").with("max", compact_tokens(max.get()))
      }
      other => budget_error(other, separator),
    })?;
    if props.used.reached(budget) {
      let step = props
        .stopped
        .as_ref()
        .map(|(step, _)| step.to_string())
        .unwrap_or_default();
      return Err(
        Message::new("tokens.errors.stepNotAboveUsed")
          .with("used", used)
          .with("step", step),
      );
    }
    Ok(budget)
  }

  fn read_time(&self, props: &RaiseLimitProps) -> Result<Duration, Message> {
    let limit = read_minutes(&self.minutes.get())?;
    if limit <= props.active {
      let used = whole_minutes(props.active);
      let key = if used == 1 {
        "errors.timeLimitNotAboveUsed.one"
      } else {
        "errors.timeLimitNotAboveUsed.other"
      };
      return Err(Message::new(key).with("used", used.to_string()));
    }
    Ok(limit)
  }

  /// "Token budget · Used 1.4M of 1.4M · Implement stopped mid-step" beside its input.
  fn budget_row(
    &self,
    ctx: &Ctx,
    props: &RaiseLimitProps,
    budget: &Result<TokenBudget, Message>,
    separator: &Arc<str>,
  ) -> Element {
    let (used, of) = (compact_tokens(props.used.get()), compact_tokens(props.budget.get()));
    let stopped = props
      .stopped
      .as_ref()
      .filter(|_| props.limit == RunLimit::Tokens || props.raises_step_budget());
    let current = match stopped {
      Some((step, _)) => ctx.t_args(
        "runs.limit.tokensUsedMidStep",
        [("used", used.as_str()), ("limit", of.as_str()), ("step", step)],
      ),
      None => ctx.t_args(
        "runs.limit.tokensUsed",
        [("used", used.as_str()), ("limit", of.as_str())],
      ),
    };
    let note = match budget {
      Ok(new) => Ok(read_back(ctx, new.get(), props.budget.get())),
      Err(error) => Err(message_text(ctx, error)),
    };
    let input = UnitInput::new(
      "raise-limit-tokens",
      self.budget.clone(),
      ctx.t("tokens.unit"),
      self.focused.clone(),
    )
    .tokens(separator.clone())
    .focus_ref(self.budget_input.clone())
    .invalid(budget.is_err());
    let name = if props.raises_step_budget() {
      ctx.t("runs.limit.stepBudget")
    } else {
      ctx.t("runs.limit.tokenBudget")
    };
    limit_row(ctx, (&name, &current), input, note)
  }

  /// "Time limit · Used 24 of 30 min of active agent time" beside its input.
  fn time_row(&self, ctx: &Ctx, props: &RaiseLimitProps, time: &Result<Duration, Message>) -> Element {
    let (used, of) = (whole_minutes(props.active), whole_minutes(props.max_duration));
    let (used, of) = (used.to_string(), of.to_string());
    let current = match props.stopped.as_ref().filter(|_| props.limit == RunLimit::Time) {
      Some((step, _)) => ctx.t_args(
        "runs.limit.timeUsedMidStep",
        [("used", used.as_str()), ("limit", of.as_str()), ("step", step)],
      ),
      None => ctx.t_args("runs.limit.timeUsed", [("used", used.as_str()), ("limit", of.as_str())]),
    };
    let note = match time {
      // Minutes left of the new limit, counted like the minutes used.
      Ok(limit) => Ok(
        ctx.t_args(
          "runs.limit.timeLeft",
          [(
            "left",
            whole_minutes(*limit)
              .saturating_sub(whole_minutes(props.active))
              .to_string()
              .as_str(),
          )],
        ),
      ),
      Err(error) => Err(message_text(ctx, error)),
    };
    let input = UnitInput::new(
      "raise-limit-time",
      self.minutes.clone(),
      ctx.t("runs.limit.minutes"),
      self.focused.clone(),
    )
    .focus_ref(self.time_input.clone())
    .invalid(time.is_err());
    limit_row(ctx, (&ctx.t("runs.limit.timeLimit"), &current), input, note)
  }
}

/// "= 2M tokens · 600k more than now".
fn read_back(ctx: &Ctx, new: u64, now: u64) -> Arc<str> {
  let count = compact_tokens(new);
  match new.cmp(&now) {
    std::cmp::Ordering::Greater => ctx.t_args(
      "tokens.readBackMore",
      [
        ("count", count.as_str()),
        ("difference", compact_tokens(new - now).as_str()),
      ],
    ),
    std::cmp::Ordering::Less => ctx.t_args(
      "tokens.readBackLess",
      [
        ("count", count.as_str()),
        ("difference", compact_tokens(now - new).as_str()),
      ],
    ),
    std::cmp::Ordering::Equal => ctx.t_args("tokens.readBack", [("count", count.as_str())]),
  }
}

/// A limit's name and use on the left, its input and read-back or error on the right (`gbyAR`).
fn limit_row(ctx: &Ctx, (name, current): (&str, &str), input: UnitInput, note: Result<Arc<str>, Arc<str>>) -> Element {
  let note: Element = match note {
    Ok(text) => Text::new(&text)
      .variant(ExtraTypography::Hint.role())
      .color(PaletteColor::TextMuted)
      .into(),
    Err(error) => wrapping_field_error(&error),
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(
      Column::new()
        .flex(1.0)
        .spacing(SpacingSize::Xs)
        .child(
          Text::new(name)
            .variant(TypographyStyle::FieldLabel)
            .color(PaletteColor::TextPrimary),
        )
        .child(Text::new(current).variant(TypographyStyle::Caption)),
    )
    .child(
      Column::new()
        .width(Dimension::Px(INPUT_WIDTH))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          input
            .size(Dimension::Pct(100.0), Dimension::Px(INPUT_HEIGHT))
            .render(ctx),
        )
        .child(note),
    )
    .into()
}

/// What continuing does (`GQIYp`).
fn explanation(ctx: &Ctx, props: &RaiseLimitProps) -> Arc<str> {
  let number = props.number.to_string();
  let checkpoint = props.checkpoint.as_deref().unwrap_or_default();
  let (with_checkpoint, without) = if props.raises_step_budget() {
    (
      "runs.limit.continueStepBudget",
      "runs.limit.continueStepBudgetNoCheckpoint",
    )
  } else {
    ("runs.limit.continueMidStep", "runs.limit.continueMidStepNoCheckpoint")
  };
  match (&props.stopped, props.checkpoint.is_some()) {
    (Some((step, agent)), true) => ctx.t_args(
      with_checkpoint,
      [
        ("agent", &**agent),
        ("checkpoint", checkpoint),
        ("step", step),
        ("run", &number),
      ],
    ),
    (Some((step, agent)), false) => ctx.t_args(without, [("agent", &**agent), ("step", step), ("run", &number)]),
    (None, _) => ctx.t_args("runs.limit.continueNextStep", [("run", &number)]),
  }
}

/// "Continue with 2M tokens" (`V5TTG`), the panel's one action; it names the new value of the
/// limit that was hit.
fn continue_button(
  ctx: &Ctx,
  props: &RaiseLimitProps,
  (budget, time): (Option<&TokenBudget>, Option<&Duration>),
  (pending, submit): (bool, Submit),
) -> Element {
  let label = match (pending, props.limit, budget, time) {
    (true, ..) => ctx.t("runs.limit.continuing"),
    (false, RunLimit::Tokens | RunLimit::StepBudget { .. }, Some(budget), _) => ctx.t_args(
      "runs.limit.continueWith",
      [("count", compact_tokens(budget.get()).as_str())],
    ),
    (false, RunLimit::Time, _, Some(time)) => {
      let minutes = whole_minutes(*time);
      let key = if minutes == 1 {
        "runs.limit.continueWithMinutes.one"
      } else {
        "runs.limit.continueWithMinutes.other"
      };
      ctx.t_args(key, [("minutes", minutes.to_string().as_str())])
    }
    (false, ..) => ctx.t("runs.limit.continue"),
  };
  let button = ActionButton::new(label)
    .primary()
    .icon(if pending { Icon::Loader } else { Icon::Play })
    .id("raise-limit-continue")
    .in_form();
  let button = match submit {
    Some(submit) => button.on_click(move || submit()),
    None => button.disabled(),
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .justify(Justify::End)
    .align_items(Alignment::Center)
    .child(button.render(ctx))
    .into()
}

fn raise_action(ctx: &mut Ctx) -> RaiseAction {
  let runners = ctx
    .use_context::<Runners>()
    .expect("the root provides Runners before mounting screens");
  ctx.future_action(move |(run, raise): (RunId, Raise)| {
    let runners = runners.clone();
    async move {
      let command = match raise {
        Raise::Limits(limits) => Command::RaiseLimit {
          run,
          budget: limits.budget,
          // Limits are whole minutes; one beyond u64 milliseconds is as good as no limit.
          max_duration_ms: u64::try_from(limits.max_duration.as_millis()).unwrap_or(u64::MAX),
        },
        Raise::StepBudget(budget) => Command::RaiseStepBudget { run, budget },
      };
      runners.control_run(command).await.map_err(|error| {
        tracing::warn!(%error, "the runner did not raise the run's limits");
        control_message(&error)
      })
    }
  })
}
