//! *Raise the limit* (`Raise the limit` `xHs82`): what was used (`Used` `w85KN`: the tokens with a meter,
//! the active agent time), the new limit's field with its read-back beside what continuing does (`Input
//! row` `SeVqB`), and the actions (`Ty1jk`): *Continue with …*, which names the new limit, *Remove the
//! limit* and what removing does. The runner's refusal is said under them.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  run::RunLimit,
  tokens::{TokenBudget, compact_tokens},
};
use orchester_workspace::{budget::whole_minutes, display, messages::Message, tasks::raise::Raise};

use super::{
  super::{
    action::{DecisionAction, state},
    divider, step_label,
  },
  Raising, Stop, Unit,
};
use crate::{
  theme::{DecisionText, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    card::{CardSurface, card},
    field::{FieldInput, field, field_error},
    icon::{Icon, icon},
    message::message_text,
    meter::{MeterTone, meter},
    unavailable::reason_line,
  },
};

/// The width of what a used value is (`ghdAJ`), and of the new limit's field (`AIvV6`).
const LABEL_WIDTH: f32 = 120.0;
const FIELD_WIDTH: f32 = 300.0;

pub struct RaiseCard<'a> {
  pub stop: &'a Stop<'a>,
  pub text: &'a Signal<String>,
  pub asked: &'a Signal<Option<Raising>>,
  pub action: &'a DecisionAction<Raising>,
  pub separator: &'a str,
  /// The project's runner is online: continuing needs it.
  pub online: bool,
}

impl RaiseCard<'_> {
  pub fn render(&self, ctx: &mut Ctx) -> Element {
    let card = card("raise-limit", CardSurface::Decision)
      .spacing(ExtraSpacing::Inset.role())
      .child(head(ctx, self.stop.limit))
      .child(self.used(ctx));
    let (Some(unit), Some(read)) = (self.stop.field(), self.stop.read(&self.text.get(), self.separator)) else {
      let reason = match self.stop.limit {
        RunLimit::StepBudget { .. } => ctx.t("decisions.limit.stepNoBudget"),
        _ => ctx.t("decisions.limit.groupReason"),
      };
      return card.child(reason_line("raise-limit-unavailable", &reason)).into();
    };
    let (pending, refused) = state(self.action, self.asked.get());
    let offline = (!self.online).then(|| ctx.t("task.controls.offline"));
    let removing = match (unit, self.stop.limit) {
      (Unit::Minutes, _) => ctx.t("decisions.limit.removeMeaning.time"),
      (Unit::Tokens, RunLimit::StepBudget { .. }) => {
        let step = self.stop.stopped.map(|step| step.name.to_string()).unwrap_or_default();
        ctx.t_args("decisions.limit.removeMeaning.step", [("step", step.as_str())])
      }
      (Unit::Tokens, _) => ctx.t("decisions.limit.removeMeaning.tokens"),
    };
    let removing = Text::new(&removing)
      .id("raise-limit-remove-meaning")
      .variant(TypographyStyle::Description)
      .color(PaletteColor::TextMuted);
    card
      .child(divider())
      .child(self.input_row(ctx, unit, read.as_ref()))
      .child(self.actions(ctx, unit, read.ok(), (pending, offline.clone())))
      .child(removing)
      .with_children(offline.map(|reason| reason_line("raise-limit-offline", &reason)))
      .with_children(refused.map(|refused| field_error("raise-limit-refused", &message_text(ctx, &refused))))
      .into()
  }

  /// What the run used of its tokens, with the meter, and of its active agent time.
  fn used(&self, ctx: &Ctx) -> Element {
    let stop = self.stop;
    let step = stop.stopped.map(|step| step.name.to_string()).unwrap_or_default();
    let tokens = stop.tokens.map(|(used, budget)| {
      let used_text = compact_tokens(used.get());
      let Some(budget) = budget else {
        let text = ctx.t_args("decisions.limit.tokensUsedNoLimit", [("used", used_text.as_str())]);
        return (text, None);
      };
      let of = compact_tokens(budget.get());
      let key = match (stop.limit, stop.stopped.is_some()) {
        (RunLimit::Tokens | RunLimit::StepBudget { .. }, true) => "decisions.limit.tokensUsedMidStep",
        _ => "decisions.limit.tokensUsed",
      };
      let text = ctx.t_args(
        key,
        [
          ("used", used_text.as_str()),
          ("limit", of.as_str()),
          ("step", step.as_str()),
        ],
      );
      let reached = used.reached(budget);
      let tone = if reached { MeterTone::Reached } else { MeterTone::Plain };
      (text, Some(meter("raise-limit-meter", (used.get(), budget.get()), tone)))
    });
    let name = match stop.limit {
      RunLimit::StepBudget { .. } => ctx.t_args("decisions.limit.stepTokens", [("step", step.as_str())]),
      _ => ctx.t("decisions.limit.tokens"),
    };
    let used_minutes = whole_minutes(stop.active).to_string();
    let time = match stop.run.run.max_duration() {
      Some(limit) => ctx.t_args(
        "decisions.limit.timeUsed",
        [("used", used_minutes), ("limit", whole_minutes(limit).to_string())],
      ),
      None => ctx.t_args("decisions.limit.timeUsedNoLimit", [("used", used_minutes)]),
    };
    let mut used = Column::new()
      .id("raise-limit-used")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm);
    if let Some((text, bar)) = tokens {
      used = used
        .child(used_row("raise-limit-tokens", &name, &text, PaletteColor::TextPrimary))
        .with_children(bar.map(|bar| {
          Row::new()
            .width(Dimension::Pct(100.0))
            .spacing(SpacingSize::Md)
            .align_items(Alignment::Center)
            .child(Column::new().width(Dimension::Px(LABEL_WIDTH)))
            .child(Column::new().flex(1.0).child(bar))
        }));
    }
    used
      .child(used_row(
        "raise-limit-time",
        &ctx.t("decisions.limit.activeTime"),
        &time,
        PaletteColor::TextSecondary,
      ))
      .into()
  }

  /// The new limit's field and its read-back, beside what continuing does.
  fn input_row(&self, ctx: &mut Ctx, unit: Unit, read: Result<&Raise, &Message>) -> Element {
    let stop = self.stop;
    let label = match (unit, stop.limit) {
      (Unit::Minutes, _) => ctx.t("decisions.limit.newTime"),
      (Unit::Tokens, RunLimit::StepBudget { .. }) => {
        let step = stop.stopped.map(|step| step.name.to_string()).unwrap_or_default();
        ctx.t_args("decisions.limit.newStepBudget", [("step", step.as_str())])
      }
      (Unit::Tokens, _) => ctx.t("decisions.limit.newBudget"),
    };
    // The token field reads as tokens by its label and read-back (`C0Qk5`); minutes say their unit.
    let input = FieldInput::new("raise-limit-input", label.clone(), self.text.clone()).mono();
    let input = match unit {
      Unit::Minutes => input.unit(ctx.t("decisions.limit.minutes")),
      Unit::Tokens => input,
    };
    let (input, hint) = match read {
      Ok(raise) => (input, self.read_back(ctx, unit, *raise)),
      Err(error) => (input.error(message_text(ctx, error)), None),
    };
    let input = match unit {
      Unit::Tokens => input.grouped(self.separator),
      Unit::Minutes => input,
    };
    let control = input.render(ctx);
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg)
      .align_items(Alignment::Start)
      .child(Column::new().width(Dimension::Px(FIELD_WIDTH)).child(field(
        "raise-limit-field",
        Some(&label),
        control,
        hint.as_deref(),
      )))
      .child(
        Text::new(&self.explanation(ctx))
          .id("raise-limit-explanation")
          .variant(ExtraTypography::Lead.role())
          .flex(1.0),
      )
      .into()
  }

  /// "= 2M tokens · 600k more than now", "= 135 min · 45 min more than now".
  fn read_back(&self, ctx: &Ctx, unit: Unit, raise: Raise) -> Option<Arc<str>> {
    let run = &self.stop.run.run;
    let minutes = unit == Unit::Minutes;
    let new = self.new_count(unit, raise)?;
    let now = match (raise, self.stop.tokens) {
      _ if minutes => run.max_duration().map(whole_minutes),
      (Raise::StepBudget(_), Some((_, before))) => before.map(TokenBudget::get),
      (Raise::StepBudget(_), None) => None,
      (Raise::Limits(_), _) => run.budget().map(TokenBudget::get),
    }
    .unwrap_or(new);
    let show = |count: u64| {
      if minutes {
        count.to_string()
      } else {
        compact_tokens(count)
      }
    };
    let (key, difference) = match (new.cmp(&now), minutes) {
      (std::cmp::Ordering::Greater, false) => ("decisions.limit.readBack.tokensMore", new - now),
      (std::cmp::Ordering::Greater, true) => ("decisions.limit.readBack.minutesMore", new - now),
      (_, false) => ("decisions.limit.readBack.tokens", 0),
      (_, true) => ("decisions.limit.readBack.minutes", 0),
    };
    Some(ctx.t_args(key, [("count", show(new)), ("difference", show(difference))]))
  }

  /// What continuing does (`mft2W`): where the run resumes, and that what follows runs under the new
  /// limit.
  fn explanation(&self, ctx: &Ctx) -> Arc<str> {
    let stop = self.stop;
    let checkpoint = stop
      .run
      .run
      .last_checkpoint()
      .filter(|_| stop.run.saved_at_pause)
      .map(|checkpoint| display::short_revision(checkpoint.revision().as_str()).to_owned());
    let Some(step) = stop.stopped else {
      return ctx.t("decisions.limit.continueNext");
    };
    let name = step_label(ctx, step);
    let agent = step
      .agent
      .as_deref()
      .map_or_else(|| ctx.t("decisions.theAgent"), Arc::from);
    let args = [
      ("step", &*name),
      ("agent", &*agent),
      ("checkpoint", checkpoint.as_deref().unwrap_or_default()),
    ];
    match (stop.limit, checkpoint.is_some()) {
      (RunLimit::StepBudget { .. }, true) => ctx.t_args("decisions.limit.continueStep", args),
      (RunLimit::StepBudget { .. }, false) => ctx.t_args("decisions.limit.continueStepUnsaved", args),
      (_, true) => ctx.t_args("decisions.limit.continueMidStep", args),
      (_, false) => ctx.t_args("decisions.limit.continueMidStepUnsaved", args),
    }
  }

  /// *Continue with …* and *Remove the limit*, which both need the runner.
  fn actions(
    &self,
    ctx: &Ctx,
    unit: Unit,
    raise: Option<Raise>,
    (pending, offline): (Option<Raising>, Option<Arc<str>>),
  ) -> Element {
    let label = match raise.and_then(|raise| self.new_value(unit, raise)) {
      Some(value) => ctx.t_args(
        match unit {
          Unit::Tokens => "decisions.limit.continueWithTokens",
          Unit::Minutes => "decisions.limit.continueWithMinutes",
        },
        [("value", value.as_str())],
      ),
      None => ctx.t("decisions.limit.continue"),
    };
    let go = ActionButton::new("raise-limit-continue", ButtonKind::Primary, label)
      .icon(Icon::Play)
      .in_form();
    let go = match (&offline, pending, raise) {
      (Some(reason), ..) => go.unavailable(reason.clone()),
      (None, Some(Raising), _) => go.loading(),
      (None, None, Some(raise)) => self.press(go, raise),
      (None, None, None) => go.disabled(),
    };
    let remove = ActionButton::new(
      "raise-limit-remove",
      ButtonKind::Secondary,
      ctx.t("decisions.limit.remove"),
    )
    .icon(Icon::Infinity);
    let remove = match (offline, pending, self.stop.remove()) {
      (Some(reason), ..) => remove.unavailable(reason),
      (None, None, Some(removed)) => self.press(remove, removed),
      (None, ..) => remove.disabled(),
    };
    Row::new()
      .id("raise-limit-actions")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(go)
      .child(remove)
      .into()
  }

  /// The new limit as *Continue with …* names it: "2M tokens", "135 min"; `None` without one.
  fn new_value(&self, unit: Unit, raise: Raise) -> Option<String> {
    let count = self.new_count(unit, raise)?;
    Some(match unit {
      Unit::Minutes => count.to_string(),
      Unit::Tokens => compact_tokens(count),
    })
  }

  /// The new limit in its unit: whole minutes or tokens; `None` when it is no limit.
  fn new_count(&self, unit: Unit, raise: Raise) -> Option<u64> {
    match (unit, raise) {
      (Unit::Minutes, Raise::Limits(limits)) => limits.max_duration.map(whole_minutes),
      (_, Raise::StepBudget(budget)) => budget.map(TokenBudget::get),
      (_, Raise::Limits(limits)) => limits.budget.map(TokenBudget::get),
    }
  }

  fn press(&self, button: ActionButton, raise: Raise) -> ActionButton {
    let (action, asked, run) = (self.action.clone(), self.asked.clone(), self.stop.run.id());
    button.on_click(move || {
      // A second press while the first is under way would replace it (lurq's `FutureAction::run`).
      if action.state().get_untracked().is_pending() {
        return;
      }
      asked.set(Some(Raising));
      action.run((Raising, raise.command(run)));
    })
  }
}

/// The card's head (`drBRt`): the gauge in `warning` and what the card raises.
fn head(ctx: &Ctx, limit: RunLimit) -> Element {
  let title = match limit {
    RunLimit::StepBudget { .. } => ctx.t("decisions.limit.stepTitle"),
    RunLimit::GroupBudget => ctx.t("decisions.limit.groupTitle"),
    _ => ctx.t("decisions.limit.title"),
  };
  Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::Gauge, IconSize::Nav, PaletteColor::Warning))
    .child(
      Text::new(&title)
        .id("raise-limit-title")
        .variant(DecisionText::CardTitle.role()),
    )
    .into()
}

/// What a used value is, and the value (`Tokens` `BN9eU`).
fn used_row(id: &str, name: &str, value: &str, color: PaletteColor) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(
      Text::new(name)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .width(Dimension::Px(LABEL_WIDTH)),
    )
    .child(
      Text::new(value)
        .id(id.to_owned())
        .variant(TypographyStyle::Description)
        .color(color)
        .flex(1.0),
    )
    .into()
}
