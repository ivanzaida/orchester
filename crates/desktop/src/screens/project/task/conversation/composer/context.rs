//! The line above the composer's box (`po1Yr` in `K1oNwn`, `l7cEtA`): what the message answers,
//! changes or continues, with × to leave a question's or a plan's mode — *Answering Builder's
//! question from 12:09*, *Changes to plan v1 · sent to Planner*, *Continue 03 Implement with your
//! instructions · attempt 3* (with *Open the decision →* while the run waits for one); and before
//! the person sends into a run that does not work now, why: the limit it stopped at (*Stopped at
//! the token limit · used 9.7k of 5k tokens*), or that none of its agents reads messages any more.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Spacer, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  conversation::Recipient,
  id::TaskId,
  run::{PauseReason, RunLimit, RunStatus},
  time::Timestamp,
  tokens::compact_tokens,
};

use super::super::{divider::step_label, nav};
use crate::{
  budget::whole_minutes,
  display,
  navigation::route::{Page, TaskPageTab},
  tasks::{
    conversation::composer::{ComposerMode, Queued},
    detail::{RunView, TaskDetail},
  },
  theme::ExtraSpacing,
  ui::{
    icon::{Icon, IconSize, icon},
    pressable::{pressable, surface},
  },
};

/// The context line of `mode` for a message to `recipient` in `detail`'s run; `leave` leaves a
/// question's or a plan's mode.
pub(super) fn context_line(
  ctx: &mut Ctx,
  (mode, detail, recipient): (&ComposerMode, &TaskDetail, Recipient),
  leave: Option<Arc<dyn Fn() + Send + Sync>>,
) -> Option<Element> {
  let run = detail.run.as_ref();
  let (glyph, text) = match mode {
    ComposerMode::Queue(Queued::Decision) => return Some(decision_line(ctx, detail.task.id())),
    ComposerMode::Answer { agent, asked_at, .. } => (
      Icon::CornerDownRight,
      ctx.t_args(
        "runs.conversation.composer.answering",
        [("agent", &**agent), ("time", display::clock(*asked_at).as_str())],
      ),
    ),
    ComposerMode::PlanChanges { agent, version } => (
      Icon::Pencil,
      ctx.t_args(
        "runs.conversation.composer.planChanges",
        [("version", version.to_string().as_str()), ("agent", &**agent)],
      ),
    ),
    ComposerMode::Continue => (Icon::Play, continue_text(ctx, run?, recipient)?),
    ComposerMode::RaiseLimit => (Icon::OctagonPause, limit_text(ctx, run?)?),
    ComposerMode::RunAgain => (Icon::Lock, ctx.t("runs.conversation.composer.noReader")),
    _ => return None,
  };
  // While the run waits for the person's decision, its other decisions stay a click away.
  let decision = run
    .filter(|run| matches!(run.run.status(), RunStatus::WaitingForPerson(_)))
    .map(|_| decision_link(ctx, detail.task.id()));
  let leave = leave.map(|leave| {
    pressable(surface().id("conversation-composer-leave"), move || leave()).child(icon(
      Icon::X,
      IconSize::Small,
      PaletteColor::TextMuted,
    ))
  });
  Some(
    Row::new()
      .width(Dimension::Pct(100.0))
      .padding_horizontal(SpacingSize::Md)
      .spacing(ExtraSpacing::Tight.role())
      .align_items(Alignment::Center)
      .child(icon(glyph, IconSize::Small, PaletteColor::TextSecondary))
      .child(
        Text::new(&text)
          .id("conversation-composer-context")
          .variant(TypographyStyle::Caption),
      )
      .child(Spacer::new().flex(1.0))
      .with_children(decision)
      .with_children(leave)
      .into(),
  )
}

/// *Continue 03 Implement with your instructions · attempt 3*: the step the message to `recipient`
/// continues the run from (the one it stopped at, for Orchestrator) and the attempt it starts.
fn continue_text(ctx: &Ctx, run: &RunView, recipient: Recipient) -> Option<Arc<str>> {
  let profile = match recipient {
    Recipient::Agent(profile) => Some(profile),
    Recipient::Orchestrator => None,
  };
  let key = run.run.recovery_step(profile)?;
  let step = run.steps.iter().find(|step| step.key == key)?;
  let attempt = run.run.attempts().len() + 1;
  Some(ctx.t_args(
    "runs.conversation.composer.continueContext",
    [
      ("step", &*step_label(ctx, step.number, &step.name)),
      ("attempt", attempt.to_string().as_str()),
    ],
  ))
}

/// The limit of its own the run stopped at, and what it used of it.
fn limit_text(ctx: &Ctx, run: &RunView) -> Option<Arc<str>> {
  let RunStatus::Paused(PauseReason::LimitReached(limit)) = run.run.status() else {
    return None;
  };
  match *limit {
    RunLimit::Tokens => Some(ctx.t_args(
      "runs.conversation.composer.stoppedTokens",
      [
        ("used", compact_tokens(run.run.used().get()).as_str()),
        ("limit", compact_tokens(run.run.budget()?.get()).as_str()),
      ],
    )),
    RunLimit::Time => Some(ctx.t_args(
      "runs.conversation.composer.stoppedTime",
      [
        (
          "used",
          whole_minutes(run.active_time(Timestamp::now())).to_string().as_str(),
        ),
        ("limit", whole_minutes(run.run.max_duration()?).to_string().as_str()),
      ],
    )),
    // This app knows no accounts; the new app words a run stopped at an account's plan limit.
    RunLimit::PlanLimit { .. } => None,
    RunLimit::StepBudget { execution } => {
      let (used, budget) = run.step_budget_use(execution)?;
      let step = run.step_of(execution)?;
      Some(ctx.t_args(
        "runs.conversation.composer.stoppedStep",
        [
          ("step", &*step.name),
          ("used", compact_tokens(used.get()).as_str()),
          ("limit", compact_tokens(budget.get()).as_str()),
        ],
      ))
    }
    RunLimit::DailyBudget | RunLimit::GroupBudget => None,
  }
}

/// The run waits for the person's decision after its retries, with no agent step to continue from:
/// what the composer's messages wait for, and *Open the decision →*.
fn decision_line(ctx: &mut Ctx, task: TaskId) -> Element {
  let link = decision_link(ctx, task);
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding_horizontal(SpacingSize::Md)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("runs.conversation.composer.deciding"))
        .id("conversation-status")
        .variant(TypographyStyle::Caption),
    )
    .child(link)
    .into()
}

/// *Open the decision →*, which opens the Summary with the decision panel.
fn decision_link(ctx: &mut Ctx, task: TaskId) -> Element {
  nav(ctx)
    .link(Page::Task(task, TaskPageTab::Summary))
    .on(surface().id("conversation-open-decision"))
    .child(Text::new(&ctx.t("conversation.openDecision")).variant(TypographyStyle::Link))
    .into()
}
