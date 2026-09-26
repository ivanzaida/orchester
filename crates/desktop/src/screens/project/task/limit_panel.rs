//! Raise the limit to continue (02.L `ZUYM0`): each limit with what the run used and an input
//! for the new limit with its read-back, what continuing does, and the one action that says the
//! result ("Continue with 2M tokens"). The input of the limit that was hit is focused. The new
//! limits apply to this run only; the budget is at most the project's maximum run budget.

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
  tokens::{TokenBudget, TokensUsed, compact_tokens, grouped},
};
use orchester_ipc::Command;

use crate::{
  budget::{minutes_text, read_budget, read_minutes, whole_minutes},
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
    unit_input::UnitInput,
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
  pub fn of(run: &RunView, limit: RunLimit, limits: ProjectLimits) -> Self {
    Self {
      run: run.id(),
      number: run.run.number().get(),
      limit,
      used: run.run.used(),
      budget: run.run.budget(),
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
    }
  }

  /// The budget offered to continue with: half again the current one, within the maximum.
  fn suggested_budget(&self) -> u64 {
    let raised = self.budget.get().saturating_add(self.budget.get() / 2);
    let rounded = raised.div_ceil(SUGGESTION_STEP).saturating_mul(SUGGESTION_STEP);
    rounded.min(self.max_budget.get()).max(self.budget.get())
  }

  /// The time limit offered, in whole minutes above the active time: after a time stop half
  /// again the current one, else unchanged.
  fn suggested_time(&self) -> Duration {
    let limit = match self.limit {
      RunLimit::Time => self.max_duration + self.max_duration / 2,
      RunLimit::Tokens => self.max_duration,
    };
    let minutes = limit
      .as_secs()
      .div_ceil(MINUTE.as_secs())
      .max(whole_minutes(self.active) + 1);
    MINUTE * u32::try_from(minutes).unwrap_or(u32::MAX)
  }
}

type RaiseAction = FutureAction<(RunId, RaisedLimits), (), Message>;

pub struct RaiseLimitPanel {
  budget: Signal<String>,
  minutes: Signal<String>,
  seeded: Signal<bool>,
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
    let submit: Submit = match (&budget, &time) {
      (Ok(budget), Ok(max_duration)) if !pending => {
        let run = props.run;
        let limits = RaisedLimits {
          budget: *budget,
          max_duration: *max_duration,
        };
        Some(Arc::new(move || action.run((run, limits))))
      }
      _ => None,
    };
    let failure = state.error.filter(|_| !pending).map(|error| message_text(ctx, &error));
    let content = Column::new()
      .id("raise-limit")
      .width(Dimension::Pct(100.0))
      .padding(ExtraSpacing::Step.role())
      .spacing(SpacingSize::Md)
      .background(PaletteColor::SurfacePanel)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .child(Text::new(&ctx.t("runs.limit.raiseTitle")).variant(ExtraTypography::CardTitle.role()))
      .child(self.budget_row(ctx, &props, &budget, &separator))
      .child(self.time_row(ctx, &props, &time))
      .child(
        Text::new(&explanation(ctx, &props))
          .variant(ExtraTypography::Detail.role())
          .width(Dimension::Pct(100.0)),
      )
      .with_children(failure.map(|error| field_error(&error)))
      .child(continue_button(ctx, budget.as_ref().ok(), pending, submit.clone()));
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
      RunLimit::Tokens => &self.budget_input,
      RunLimit::Time => &self.time_input,
    });
  }

  fn read_budget(&self, props: &RaiseLimitProps, separator: &str) -> Result<TokenBudget, Message> {
    let budget = read_budget(&self.budget.get(), separator, props.max_budget)?;
    if props.used.reached(budget) {
      return Err(Message::new("tokens.errors.notAboveUsed").with("used", compact_tokens(props.used.get())));
    }
    Ok(budget)
  }

  fn read_time(&self, props: &RaiseLimitProps) -> Result<Duration, Message> {
    let limit = read_minutes(&self.minutes.get())?;
    if limit <= props.active {
      return Err(Message::new("errors.timeLimitNotAboveUsed").with("used", whole_minutes(props.active).to_string()));
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
    let current = match &props.stopped {
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
    let input = UnitInput::new("raise-limit-tokens", self.budget.clone(), ctx.t("tokens.unit"))
      .tokens(separator.clone())
      .focus_ref(self.budget_input.clone())
      .invalid(budget.is_err());
    limit_row(ctx, (&ctx.t("runs.limit.tokenBudget"), &current), input, note)
  }

  /// "Time limit · Used 24 of 30 min of active agent time" beside its input.
  fn time_row(&self, ctx: &Ctx, props: &RaiseLimitProps, time: &Result<Duration, Message>) -> Element {
    let current = ctx.t_args(
      "runs.limit.timeUsed",
      [
        ("used", whole_minutes(props.active).to_string().as_str()),
        ("limit", whole_minutes(props.max_duration).to_string().as_str()),
      ],
    );
    let note = match time {
      Ok(limit) => Ok(ctx.t_args(
        "runs.limit.timeLeft",
        [(
          "left",
          whole_minutes(limit.saturating_sub(props.active)).to_string().as_str(),
        )],
      )),
      Err(error) => Err(message_text(ctx, error)),
    };
    let input = UnitInput::new("raise-limit-time", self.minutes.clone(), ctx.t("runs.limit.minutes"))
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
  match (&props.stopped, props.checkpoint.is_some()) {
    (Some((step, agent)), true) => ctx.t_args(
      "runs.limit.continueMidStep",
      [
        ("agent", &**agent),
        ("checkpoint", checkpoint),
        ("step", step),
        ("run", &number),
      ],
    ),
    (Some((step, agent)), false) => ctx.t_args(
      "runs.limit.continueMidStepNoCheckpoint",
      [("agent", &**agent), ("step", step), ("run", &number)],
    ),
    (None, _) => ctx.t_args("runs.limit.continueNextStep", [("run", &number)]),
  }
}

/// "Continue with 2M tokens" (`V5TTG`), the panel's one action.
fn continue_button(ctx: &Ctx, budget: Option<&TokenBudget>, pending: bool, submit: Submit) -> Element {
  let label = match (pending, budget) {
    (true, _) => ctx.t("runs.limit.continuing"),
    (false, Some(budget)) => ctx.t_args(
      "runs.limit.continueWith",
      [("count", compact_tokens(budget.get()).as_str())],
    ),
    (false, None) => ctx.t("runs.limit.continue"),
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
  ctx.future_action(move |(run, limits): (RunId, RaisedLimits)| {
    let runners = runners.clone();
    async move {
      let command = Command::RaiseLimit {
        run,
        budget: limits.budget,
        // Limits are whole minutes; one beyond u64 milliseconds is as good as no limit.
        max_duration_ms: u64::try_from(limits.max_duration.as_millis()).unwrap_or(u64::MAX),
      };
      runners.control_run(command).await.map_err(|error| {
        tracing::warn!(%error, "the runner did not raise the run's limits");
        control_message(&error)
      })
    }
  })
}
