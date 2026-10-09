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
use orchester_core::{run::RunLimit, tokens::compact_tokens};
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
}

impl RaiseCard<'_> {
  pub fn render(&self, ctx: &mut Ctx) -> Element {
    let card = card("raise-limit", CardSurface::Decision)
      .spacing(ExtraSpacing::Inset.role())
      .child(head(ctx, self.stop.limit))
      .child(self.used(ctx));
    let (Some(unit), Some(read)) = (self.stop.field(), self.stop.read(&self.text.get(), self.separator)) else {
      let reason = ctx.t("decisions.limit.groupReason");
      return card.child(reason_line("raise-limit-unavailable", &reason)).into();
    };
    let (pending, refused) = state(self.action, self.asked.get());
    card
      .child(divider())
      .child(self.input_row(ctx, unit, read.as_ref()))
      .child(self.actions(ctx, unit, read.ok(), pending))
      .with_children(refused.map(|refused| field_error("raise-limit-refused", &message_text(ctx, &refused))))
      .into()
  }

  /// What the run used of its tokens, with the meter, and of its active agent time.
  fn used(&self, ctx: &Ctx) -> Element {
    let stop = self.stop;
    let step = stop.stopped.map(|step| step.name.to_string()).unwrap_or_default();
    let tokens = stop.tokens.map(|(used, budget)| {
      let (used_text, of) = (compact_tokens(used.get()), compact_tokens(budget.get()));
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
      (text, meter("raise-limit-meter", (used.get(), budget.get()), tone))
    });
    let name = match stop.limit {
      RunLimit::StepBudget { .. } => ctx.t_args("decisions.limit.stepTokens", [("step", step.as_str())]),
      _ => ctx.t("decisions.limit.tokens"),
    };
    let (used_minutes, of) = (whole_minutes(stop.active), whole_minutes(stop.run.run.max_duration()));
    let time = ctx.t_args(
      "decisions.limit.timeUsed",
      [("used", used_minutes.to_string()), ("limit", of.to_string())],
    );
    let mut used = Column::new()
      .id("raise-limit-used")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm);
    if let Some((text, bar)) = tokens {
      used = used
        .child(used_row("raise-limit-tokens", &name, &text, PaletteColor::TextPrimary))
        .child(
          Row::new()
            .width(Dimension::Pct(100.0))
            .spacing(SpacingSize::Md)
            .align_items(Alignment::Center)
            .child(Column::new().width(Dimension::Px(LABEL_WIDTH)))
            .child(Column::new().flex(1.0).child(bar)),
        );
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
    let (label, unit_word) = match (unit, stop.limit) {
      (Unit::Minutes, _) => (ctx.t("decisions.limit.newTime"), ctx.t("decisions.limit.minutes")),
      (Unit::Tokens, RunLimit::StepBudget { .. }) => {
        let step = stop.stopped.map(|step| step.name.to_string()).unwrap_or_default();
        (
          ctx.t_args("decisions.limit.newStepBudget", [("step", step.as_str())]),
          ctx.t("decisions.limit.tokensUnit"),
        )
      }
      (Unit::Tokens, _) => (ctx.t("decisions.limit.newBudget"), ctx.t("decisions.limit.tokensUnit")),
    };
    let input = FieldInput::new("raise-limit-input", label.clone(), self.text.clone())
      .mono()
      .unit(unit_word);
    let (input, hint) = match read {
      Ok(raise) => (input, Some(self.read_back(ctx, *raise))),
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
  fn read_back(&self, ctx: &Ctx, raise: Raise) -> Arc<str> {
    let run = &self.stop.run.run;
    let (new, now, minutes) = match (raise, self.stop.tokens) {
      (Raise::StepBudget(budget), Some((_, before))) => (budget.get(), before.get(), false),
      (Raise::Limits(limits), _) if self.stop.limit == RunLimit::Time => (
        whole_minutes(limits.max_duration),
        whole_minutes(run.max_duration()),
        true,
      ),
      (Raise::Limits(limits), _) => (limits.budget.get(), run.budget().get(), false),
      (Raise::StepBudget(budget), None) => (budget.get(), budget.get(), false),
    };
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
    ctx.t_args(key, [("count", show(new)), ("difference", show(difference))])
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

  /// *Continue with …*, *Remove the limit* and what removing does.
  fn actions(&self, ctx: &Ctx, unit: Unit, raise: Option<Raise>, pending: Option<Raising>) -> Element {
    let label = match raise.map(|raise| self.new_value(unit, raise)) {
      Some(value) => ctx.t_args("decisions.limit.continueWith", [("value", value.as_str())]),
      None => ctx.t("decisions.limit.continue"),
    };
    let go = ActionButton::new("raise-limit-continue", ButtonKind::Primary, label)
      .icon(Icon::Play)
      .in_form();
    let remove = ActionButton::new(
      "raise-limit-remove",
      ButtonKind::Secondary,
      ctx.t("decisions.limit.remove"),
    )
    .icon(Icon::Infinity)
    .in_form();
    let (go, remove) = match pending {
      Some(Raising::Continue) => (go.loading(), remove.disabled()),
      Some(Raising::Remove) => (go.disabled(), remove.loading()),
      None => (
        match raise {
          Some(raise) => self.press(go, Raising::Continue, raise),
          None => go.disabled(),
        },
        self.press(remove, Raising::Remove, self.stop.removed()),
      ),
    };
    let hint = match (unit, self.stop.limit) {
      (Unit::Minutes, _) => ctx.t("decisions.limit.removeTime"),
      (Unit::Tokens, RunLimit::StepBudget { .. }) => ctx.t("decisions.limit.removeStepBudget"),
      (Unit::Tokens, _) => ctx.t("decisions.limit.removeTokens"),
    };
    Row::new()
      .id("raise-limit-actions")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(go)
      .child(remove)
      .child(
        Text::new(&hint)
          .id("raise-limit-remove-hint")
          .variant(ExtraTypography::Note.role())
          .flex(1.0),
      )
      .into()
  }

  /// The new limit as *Continue with …* names it: "2M tokens", "135 min".
  fn new_value(&self, unit: Unit, raise: Raise) -> String {
    match (unit, raise) {
      (Unit::Minutes, Raise::Limits(limits)) => whole_minutes(limits.max_duration).to_string(),
      (_, Raise::StepBudget(budget)) => compact_tokens(budget.get()),
      (_, Raise::Limits(limits)) => compact_tokens(limits.budget.get()),
    }
  }

  fn press(&self, button: ActionButton, raising: Raising, raise: Raise) -> ActionButton {
    let (action, asked, run) = (self.action.clone(), self.asked.clone(), self.stop.run.id());
    button.on_click(move || {
      asked.set(Some(raising));
      action.run((raising, raise.command(run)));
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
