//! *Decide how the run continues* (`Decision` `j7DjNG`): the tokens the run used and has left, *Allow 1 retry
//! & continue* with what it runs, returning the work to an agent step (`Return the work to` `qrV0F`) with
//! the note its agent reads (`Note for Builder · required` `wJTjt`), and *Stop and fail the run*. A
//! decision under way holds the others; the runner's refusal is said above *Stop*. Without an agent step
//! to return to, that part is left out.

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::{Column, SelectOption, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  resource::StepKey,
  run::{ReviewNote, Stuck, StuckDecision},
  tokens::compact_tokens,
};
use orchester_ipc::Command;
use orchester_workspace::{budget::unbounded, tasks::detail::RunView};

use super::{
  super::{
    action::{DecisionAction, state},
    divider, step_label,
  },
  Deciding, Target,
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, TaskText},
  ui::{
    button::{ActionButton, ButtonKind},
    card::{CardSurface, card},
    detail_row::{DetailValue, detail_row},
    field::{FieldInput, SelectField, field, field_error},
    icon::Icon,
    message::message_text,
    unavailable::reason_line,
  },
};

pub struct DecideCard<'a> {
  pub run: &'a RunView,
  pub stuck: &'a Stuck,
  pub target: &'a Signal<Target>,
  pub note: &'a Signal<String>,
  pub asked: &'a Signal<Option<Deciding>>,
  pub action: &'a DecisionAction<Deciding>,
  /// Why nothing can be decided now: no runner is connected to carry it out.
  pub offline: Option<Arc<str>>,
}

impl DecideCard<'_> {
  pub fn render(&self, ctx: &mut Ctx) -> Element {
    let (pending, refused) = state(self.action, self.asked.get());
    let retry = self
      .button(
        ActionButton::new(
          "task-allow-retry",
          ButtonKind::Primary,
          ctx.t("decisions.retries.allow"),
        )
        .icon(Icon::RotateCcw)
        .in_form(),
        StuckDecision::Retry,
        pending,
      )
      .fill_width();
    let stop = self
      .button(
        ActionButton::new("task-stop-run", ButtonKind::Danger, ctx.t("decisions.retries.stop"))
          .icon(Icon::OctagonX)
          .in_form(),
        StuckDecision::Stop,
        pending,
      )
      .fill_width();
    card("task-retry-decision", CardSurface::Attention)
      .spacing(ExtraSpacing::Loose.role())
      .child(
        Text::new(&ctx.t("decisions.retries.title"))
          .id("task-retry-decision-title")
          .variant(TaskText::SectionTitle.role()),
      )
      .child(self.facts(ctx))
      .child(retry)
      .child(
        Text::new(&self.retry_meaning(ctx))
          .id("task-retry-meaning")
          .variant(ExtraTypography::Note.role())
          .width(Dimension::Pct(100.0)),
      )
      .with_children(self.returning(ctx, pending))
      .child(divider())
      .with_children(
        self
          .offline
          .as_ref()
          .map(|reason| reason_line("task-retry-offline", reason)),
      )
      .with_children(refused.map(|refused| field_error("task-retry-refused", &message_text(ctx, &refused))))
      .child(stop)
      .into()
  }

  /// "Tokens used 1.1M", "Remaining budget No limit".
  fn facts(&self, ctx: &Ctx) -> Element {
    let run = &self.run.run;
    let left = if run.budget() == unbounded() {
      ctx.t("decisions.retries.noLimit")
    } else {
      ctx.t_args(
        "decisions.retries.tokens",
        [("count", compact_tokens(run.used().left_of(run.budget())))],
      )
    };
    Column::new()
      .width(Dimension::Pct(100.0))
      .child(detail_row(
        "task-retry-used",
        &ctx.t("decisions.retries.used"),
        &compact_tokens(run.used().get()),
        DetailValue::Text,
      ))
      .child(detail_row(
        "task-retry-left",
        &ctx.t("decisions.retries.remaining"),
        &left,
        DetailValue::Text,
      ))
      .into()
  }

  /// What one more retry runs: the step it resumes, and the failing step after it.
  fn retry_meaning(&self, ctx: &Ctx) -> Arc<str> {
    let failed = self.name(ctx, self.stuck.step());
    match self.stuck {
      Stuck::RetriesExhausted(step) => {
        let from = self.name(ctx, self.run.run.retry_from().unwrap_or(step));
        ctx.t_args(
          "decisions.retries.retryMeaning",
          [("step", &*from), ("failed", &*failed)],
        )
      }
      Stuck::BrokeAgain(_) => ctx.t_args("decisions.retries.restartMeaning", [("step", &*failed)]),
    }
  }

  /// The step choice, the note and *Return to … with note*, after a divider; none without a step to
  /// return to.
  fn returning(&self, ctx: &mut Ctx, pending: Option<Deciding>) -> Vec<Element> {
    let targets = self.run.run.return_targets();
    if targets.is_empty() {
      return Vec::new();
    }
    let default = self.run.run.retry_from();
    let options: Vec<SelectOption<Target>> = targets
      .iter()
      .map(|key| {
        let name = self.name(ctx, key);
        let label = if Some(key) == default {
          ctx.t_args("decisions.retries.defaultTarget", [("step", &*name)])
        } else {
          name
        };
        SelectOption::new(Target(Some(key.clone())), label)
      })
      .collect();
    let chosen = self.target.get().0.filter(|key| targets.contains(key));
    let agent = chosen
      .as_ref()
      .and_then(|key| self.run.steps.iter().find(|step| step.key == *key))
      .and_then(|step| step.agent.clone())
      .unwrap_or_else(|| ctx.t("decisions.theAgent"));
    let to = ctx.t("decisions.retries.returnTo");
    let select = SelectField::new("task-return-step", to.clone(), self.target.clone(), options).render(ctx);
    let note_label = ctx.t_args("decisions.retries.noteFor", [("agent", &*agent)]);
    let note = FieldInput::new("task-return-note", note_label.clone(), self.note.clone())
      .text_area()
      .placeholder(ctx.t_args("decisions.retries.notePlaceholder", [("agent", &*agent)]))
      .render(ctx);
    let step = chosen.as_ref().map(|key| self.short_name(key)).unwrap_or_default();
    let returning = ActionButton::new(
      "task-return-with-note",
      ButtonKind::Secondary,
      ctx.t_args("decisions.retries.returnWithNote", [("step", step.as_str())]),
    )
    .icon(Icon::CornerDownLeft)
    .in_form()
    .fill_width();
    let decision = chosen
      .zip(ReviewNote::new(&self.note.get()).ok())
      .map(|(step, note)| StuckDecision::ReturnTo { step, note });
    let returning = match decision {
      Some(decision) => self.button(returning, decision, pending),
      None => match &self.offline {
        Some(reason) => returning.unavailable(reason.clone()),
        None if pending == Some(Deciding::Return) => returning.loading(),
        None => returning.disabled(),
      },
    };
    vec![
      divider(),
      field("task-return-step-field", Some(&to), select, None).into(),
      field("task-return-note-field", Some(&note_label), note, None).into(),
      returning.into(),
    ]
  }

  /// `button` asking for `decision`: under way while it is, waiting while another is.
  fn button(&self, button: ActionButton, decision: StuckDecision, pending: Option<Deciding>) -> ActionButton {
    let deciding = Deciding::of(&decision);
    if let Some(reason) = &self.offline {
      return button.unavailable(reason.clone());
    }
    match pending {
      Some(asked) if asked == deciding => button.loading(),
      Some(_) => button.disabled(),
      None => {
        let (action, asked, run) = (self.action.clone(), self.asked.clone(), self.run.id());
        button.on_click(move || {
          // A second press while the first is under way would replace it (lurq's `FutureAction::run`).
          if action.state().get_untracked().is_pending() {
            return;
          }
          asked.set(Some(deciding));
          action.run((
            deciding,
            Command::DecideStuckRun {
              run,
              decision: decision.clone(),
            },
          ));
        })
      }
    }
  }

  /// "03 Implement", or the key of a step the run no longer lists.
  fn name(&self, ctx: &Ctx, key: &StepKey) -> Arc<str> {
    self
      .run
      .steps
      .iter()
      .find(|step| step.key == *key)
      .map_or_else(|| Arc::from(key.as_str()), |step| step_label(ctx, step))
  }

  /// The step's name alone, as *Return to Implement with note* says it.
  fn short_name(&self, key: &StepKey) -> String {
    self
      .run
      .steps
      .iter()
      .find(|step| step.key == *key)
      .map_or_else(|| key.as_str().to_owned(), |step| step.name.to_string())
  }
}
