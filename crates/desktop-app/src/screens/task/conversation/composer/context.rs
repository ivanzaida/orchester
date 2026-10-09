//! The line over the composer's box (`Context line` `B5qynx`, `X4r07C` in `YhiGC`): 32 high, `surface_raised`
//! in a `border` stroke, radius 8, a 13 icon and the words in 12 `text_secondary`, 8 apart, 10 in from its
//! start and 6 from its end. It says what the message answers, changes or continues — *Answering Builder's
//! question from 16:42*, *Changes to plan v1 · Planner writes plan v2 from your note*, *Continue 03
//! Implement with your instructions · attempt 3* — with × (a 26 square, a 15 glyph) to leave a question's or
//! a plan's mode; and before the person sends into a run that does not work now, why: the limit it stopped
//! at, that the run waits for a decision (*Open the decision →*), or that none of its agents reads messages.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::{
  conversation::Recipient,
  run::{PauseReason, RunLimit, RunStatus},
  time::Timestamp,
  tokens::compact_tokens,
};
use orchester_workspace::{
  budget::whole_minutes,
  display,
  tasks::{
    conversation::composer::{ComposerMode, Queued},
    detail::RunView,
  },
};

use super::{super::timeline::step_label, Composing, Press};
use crate::{
  routes::TaskView,
  theme::{ComposerRadius, ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
    tooltip::Tooltip,
  },
};

/// The line's height (`B5qynx`) and its × (`TOQ85`).
const LINE_HEIGHT: f32 = 32.0;
const LEAVE_SIZE: f32 = 26.0;

/// The line of `mode` for a message to `recipient`; `leave` leaves a question's or a plan's mode.
pub(super) fn line(
  ctx: &mut Ctx,
  composing: &Composing,
  (mode, recipient): (&ComposerMode, Recipient),
  leave: Option<Press>,
) -> Option<Element> {
  let run = composing.detail.run.as_ref();
  let (glyph, words) = match mode {
    ComposerMode::Queue(Queued::Decision) => (Icon::OctagonPause, ctx.t("composer.context.deciding")),
    ComposerMode::Answer { agent, asked_at, .. } => (
      Icon::Reply,
      ctx.t_args(
        "composer.context.answering",
        [("agent", &**agent), ("time", display::clock(*asked_at).as_str())],
      ),
    ),
    ComposerMode::PlanChanges { agent, version } => (
      Icon::PencilLine,
      ctx.t_args(
        "composer.context.planChanges",
        [
          ("version", version.get().to_string().as_str()),
          ("agent", &**agent),
          ("next", version.next().get().to_string().as_str()),
        ],
      ),
    ),
    ComposerMode::Continue => (Icon::Play, continue_words(ctx, run?, recipient)?),
    ComposerMode::RaiseLimit => (Icon::OctagonPause, limit_words(ctx, run?)?),
    ComposerMode::RunAgain => (Icon::Lock, ctx.t("composer.context.noReader")),
    _ => return None,
  };
  // While the run waits for the person's decision, its Summary is a press away.
  let decision = run
    .filter(|run| matches!(run.run.status(), RunStatus::WaitingForPerson(_)))
    .map(|_| decision_link(ctx, composing));
  let leave = leave.map(|leave| leave_button(ctx, leave));
  Some(
    Row::new()
      .id("composer-context")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(LINE_HEIGHT))
      .padding_left(ExtraSpacing::Loose.role())
      .padding_right(ExtraSpacing::Snug.role())
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(ComposerRadius::Context.role())
      .child(icon(glyph, IconSize::Base, PaletteColor::TextSecondary))
      .child(
        Text::new(&words)
          .id("composer-context-text")
          .variant(TypographyStyle::Description)
          .flex(1.0)
          .min_width(Dimension::Px(0.0)),
      )
      .with_children(decision)
      .with_children(leave)
      .into(),
  )
}

/// *Continue 03 Implement with your instructions · attempt 3*: the step the message to `recipient`
/// continues the run from (the one it stopped at, for Orchestrator) and the attempt it starts.
fn continue_words(ctx: &Ctx, run: &RunView, recipient: Recipient) -> Option<Arc<str>> {
  let profile = match recipient {
    Recipient::Agent(profile) => Some(profile),
    Recipient::Orchestrator => None,
  };
  let key = run.run.recovery_step(profile)?;
  let step = run.steps.iter().find(|step| step.key == key)?;
  let attempt = run.run.attempts().len() + 1;
  Some(ctx.t_args(
    "composer.context.continue",
    [
      ("step", &*step_label(ctx, step.number, &step.name)),
      ("attempt", attempt.to_string().as_str()),
    ],
  ))
}

/// The limit of its own the run stopped at, and what it used of it.
fn limit_words(ctx: &Ctx, run: &RunView) -> Option<Arc<str>> {
  let RunStatus::Paused(PauseReason::LimitReached(limit)) = run.run.status() else {
    return None;
  };
  match *limit {
    RunLimit::Tokens => Some(ctx.t_args(
      "composer.context.stoppedTokens",
      [
        ("used", compact_tokens(run.run.used().get()).as_str()),
        ("limit", compact_tokens(run.run.budget().get()).as_str()),
      ],
    )),
    RunLimit::Time => Some(ctx.t_args(
      "composer.context.stoppedTime",
      [
        (
          "used",
          whole_minutes(run.active_time(Timestamp::now())).to_string().as_str(),
        ),
        ("limit", whole_minutes(run.run.max_duration()).to_string().as_str()),
      ],
    )),
    RunLimit::StepBudget { execution } => {
      let (used, budget) = run.step_budget_use(execution)?;
      let step = run.step_of(execution)?;
      Some(ctx.t_args(
        "composer.context.stoppedStep",
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

/// *Open the decision →*: the task's Summary, where the run's decision waits.
fn decision_link(ctx: &mut Ctx, composing: &Composing) -> Element {
  let words = ctx.t("composer.context.openDecision");
  pressable(composing.open(ctx, TaskView::Summary))
    .id("composer-open-decision")
    .child(
      Text::new(&words)
        .variant(ExtraTypography::NoteStrong.role())
        .color(ExtraColor::AccentText.role())
        .nowrap(),
    )
    .into()
}

/// ×: the message stops answering the question or changing the plan.
fn leave_button(ctx: &mut Ctx, leave: Press) -> Element {
  let words = ctx.t("composer.context.leave");
  let button = pressable(move || leave())
    .id("composer-leave")
    .describe("a11y_name", words.clone())
    .size(Dimension::Px(LEAVE_SIZE), Dimension::Px(LEAVE_SIZE))
    .flex_shrink(0.0)
    .justify(Justify::Center)
    .rounded(lurq::app::theme::RadiusSize::Md)
    .cursor(CursorIcon::Pointer)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(icon(Icon::X, IconSize::Nav, PaletteColor::TextSecondary));
  Tooltip::new("composer-leave", words).wrap(ctx, button)
}
