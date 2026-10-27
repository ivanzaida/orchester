//! The run's steps on a rail (`Steps` `Utjda` in `Progress` `ee0vh`): a chip per step in stage order, parallel steps
//! that have not started included, 8 apart with a 1 `border` line between them. A chip is the state's 13 icon in its
//! tone, the step's number in Geist Mono 11 `text_muted` and its name in 12, 6 apart, 6 over and under and 10 at its
//! sides, radius 6: in `text_secondary`, `text_muted` before it starts, and the step the run stands at in 12/500
//! `text_primary` on `warning_muted`. Its tooltip says what the step's row said: its agent, where it stands with its
//! attempt, the tokens it used and its active agent time ("Builder · Complete · attempt 1 · 41k tokens · 6 min"),
//! counting on while its agent works; a step whose agent waits for the person's approval says so and since when.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{run::StepFailure, time::Timestamp, tokens::compact_tokens};
use orchester_workspace::{
  display,
  tasks::{
    detail::RunView,
    progress::{ProgressRow, ProgressState, progress},
  },
};

use super::live::Ticker;
use crate::{
  theme::{ControlHeight, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    message::message_text,
    status::Status,
    tooltip::Tooltip,
  },
};

/// One step's chip: its number in the pipeline, its progress, and since when its agent waits for the person's
/// approval.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepRow {
  number: usize,
  progress: ProgressRow,
  waits_since: Option<Timestamp>,
}

/// The rail of `run`'s steps at `now`, the step with key `current` drawn as the one the run stands at. `lost`: the run
/// is recorded as running but has no runner; `waiting`: the steps, by key, whose agent waits for an approval, and
/// since when.
pub fn rail(
  ctx: &mut Ctx,
  run: &RunView,
  (lost, waiting): (bool, &[(Arc<str>, Timestamp)]),
  current: Option<Arc<str>>,
  now: Timestamp,
) -> Element {
  let rows = rows(run, lost, waiting, now);
  ctx.mount_keyed::<LiveRail>("task-steps-rail", RailProps { rows, current })
}

/// The rows of `run`'s steps at `now`, in stage order; `lost` and `waiting` as for [`rail`].
fn rows(run: &RunView, lost: bool, waiting: &[(Arc<str>, Timestamp)], now: Timestamp) -> Vec<StepRow> {
  progress(run, lost, now)
    .into_iter()
    .map(|progress| without_runner(progress, lost))
    .map(|progress| StepRow {
      number: run
        .steps
        .iter()
        .find(|step| *step.key.as_str() == *progress.id)
        .map_or(0, |step| step.number),
      waits_since: waiting
        .iter()
        .find(|(step, _)| *step == progress.id)
        .map(|(_, since)| *since),
      progress,
    })
    .collect()
}

#[derive(Clone, PartialEq)]
struct RailProps {
  rows: Vec<StepRow>,
  current: Option<Arc<str>>,
}

impl lurq::app::component::DevtoolsInspectable for RailProps {}

/// The rail, its tooltips' active agent time going on while their agents work.
struct LiveRail {
  ticker: Ticker,
}

impl Component for LiveRail {
  type Props = RailProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      ticker: Ticker::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let RailProps { rows, current } = ctx.props::<RailProps>().clone();
    let counting = rows
      .iter()
      .any(|row| row.progress.usage.is_some_and(|usage| usage.active.counting()));
    let now = self.ticker.now(ctx, counting);
    let line = ctx.theme().border_size_value(BorderSize::Sm);
    let mut rail = Row::new()
      .id("task-steps")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center);
    for (index, row) in rows.iter().enumerate() {
      if index > 0 {
        rail = rail.child(
          Row::new()
            .flex(1.0)
            .min_width(Dimension::Px(0.0))
            .height(Dimension::Px(line))
            .background(PaletteColor::Border),
        );
      }
      let here = current.as_deref() == Some(row.key());
      rail = rail.child(chip(ctx, row, here, now));
    }
    rail
  }
}

impl StepRow {
  /// The key of the row's step.
  fn key(&self) -> &str {
    &self.progress.id
  }
}

/// A step's chip, `here` when the run stands at it, with what it did in its tooltip.
fn chip(ctx: &mut Ctx, row: &StepRow, here: bool, now: Timestamp) -> Element {
  let progress = &row.progress;
  let (glyph, color) = match (row.waits_since, unjudged(progress.state)) {
    (None, Some(glyph)) => (glyph, PaletteColor::Warning),
    (Some(_), _) => (Status::NeedsInput.icon(), Status::NeedsInput.color()),
    (None, None) => (status(progress.state).icon(), status(progress.state).color()),
  };
  let (typography, name_color) = match (here, progress.state) {
    (true, _) => (TypographyStyle::Label, PaletteColor::TextPrimary),
    (false, ProgressState::Waiting) => (TypographyStyle::Description, PaletteColor::TextMuted),
    (false, _) => (TypographyStyle::Description, PaletteColor::TextSecondary),
  };
  let detail = detail(ctx, row, now);
  let id = format!("task-step-{}", progress.id);
  let chip = Row::new()
    .id(id.clone())
    .flex_shrink(0.0)
    .height(Dimension::Px(ControlHeight::Md.value()))
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .describe("step", progress.id.to_string())
    .describe("state", format!("{:?}", progress.state))
    .describe("current", if here { "true" } else { "false" })
    .describe("detail", detail.clone())
    .child(icon(glyph, IconSize::Base, color))
    .child(
      Text::new(&format!("{:02}", row.number))
        .variant(ExtraTypography::MonoNote.role())
        .nowrap(),
    )
    .child(
      Text::new(&progress.step)
        .id(format!("{id}-name"))
        .variant(typography)
        .color(name_color)
        .nowrap(),
    );
  let chip = if here {
    chip.background(PaletteColor::WarningMuted)
  } else {
    chip
  };
  Tooltip::new(format!("{id}-tip"), detail).wrap(ctx, chip)
}

/// A step of a run without a runner (`lost`) that was at work or waited for the person is interrupted:
/// nothing works for it and nothing waits on it until a runner is back.
fn without_runner(mut row: ProgressRow, lost: bool) -> ProgressRow {
  if lost && matches!(row.state, ProgressState::Working | ProgressState::NeedsYou) {
    row.state = ProgressState::Interrupted;
  }
  row
}

/// The label a step's state reads as.
const fn status(state: ProgressState) -> Status {
  match state {
    ProgressState::Waiting => Status::NotStarted,
    ProgressState::Working => Status::Working,
    ProgressState::NeedsYou => Status::NeedsInput,
    ProgressState::Paused => Status::Paused,
    ProgressState::StoppedAtLimit => Status::StoppedAtLimit,
    ProgressState::Interrupted => Status::Interrupted,
    ProgressState::CantContinue => Status::CantContinue,
    ProgressState::Complete => Status::Complete,
    ProgressState::Failed(_) => Status::Failed,
    ProgressState::Skipped => Status::Skipped,
    ProgressState::Cancelled => Status::Canceled,
  }
}

/// The icon of a step that ended without a verdict on its work, for the person to decide: it could not check
/// everything (`circle-dashed`) or was not run again on an unchanged revision (`hand`), as the run's line says it.
const fn unjudged(state: ProgressState) -> Option<Icon> {
  match state {
    ProgressState::Failed(Some(StepFailure::Unverified)) => Some(Icon::CircleDashed),
    ProgressState::Failed(Some(StepFailure::Unchanged)) => Some(Icon::Hand),
    _ => None,
  }
}

/// Who works on a step and where it stands, with its attempt, tokens and active agent time at `now` when it ran.
fn detail(ctx: &Ctx, row: &StepRow, now: Timestamp) -> Arc<str> {
  let stands = stands(ctx, row, now);
  match row.progress.agent.as_deref() {
    Some(agent) => ctx.t_args("summary.steps.byAgent", [("agent", agent), ("detail", &*stands)]),
    None => stands,
  }
}

/// Where a step stands, with its attempt, tokens and active agent time at `now` when it ran.
fn stands(ctx: &Ctx, row: &StepRow, now: Timestamp) -> Arc<str> {
  if let Some(since) = row.waits_since {
    return ctx.t_args("summary.steps.waitsForApproval", [("time", display::clock(since))]);
  }
  let state = ctx.t(state_key(row.progress.state));
  let Some(usage) = row.progress.usage else {
    return state;
  };
  let attempt = usage.attempt.to_string();
  let duration = message_text(ctx, &display::duration(usage.active.at(now)));
  match usage.tokens {
    Some(tokens) => ctx.t_args(
      "summary.steps.detail",
      [
        ("state", &*state),
        ("attempt", attempt.as_str()),
        ("tokens", compact_tokens(tokens).as_str()),
        ("duration", &*duration),
      ],
    ),
    // A tool step's command uses no tokens.
    None => ctx.t_args(
      "summary.steps.commandDetail",
      [
        ("state", &*state),
        ("attempt", attempt.as_str()),
        ("duration", &*duration),
      ],
    ),
  }
}

const fn state_key(state: ProgressState) -> &'static str {
  match state {
    ProgressState::Waiting => "summary.steps.state.notStarted",
    ProgressState::Working => "summary.steps.state.working",
    ProgressState::NeedsYou => "summary.steps.state.needsYou",
    ProgressState::Paused => "summary.steps.state.paused",
    ProgressState::StoppedAtLimit => "summary.steps.state.stoppedAtLimit",
    ProgressState::Interrupted => "summary.steps.state.interrupted",
    ProgressState::CantContinue => "summary.steps.state.cantContinue",
    ProgressState::Complete => "summary.steps.state.complete",
    ProgressState::Failed(failure) => failure_key(failure),
    ProgressState::Skipped => "summary.steps.state.skipped",
    ProgressState::Cancelled => "summary.steps.state.canceled",
  }
}

/// Why a step failed, in a word or two.
const fn failure_key(failure: Option<StepFailure>) -> &'static str {
  match failure {
    None | Some(StepFailure::AgentFailed) => "summary.steps.state.failed",
    Some(StepFailure::ReportedFailed) => "summary.steps.failure.reported",
    Some(StepFailure::NotStarted) => "summary.steps.failure.notStarted",
    Some(StepFailure::StepBudget) => "summary.steps.failure.stepBudget",
    Some(StepFailure::MergeConflict) => "summary.steps.failure.mergeConflict",
    Some(StepFailure::CheckpointFailed) => "summary.steps.failure.checkpointFailed",
    Some(StepFailure::CommandFailed) => "summary.steps.failure.commandFailed",
    Some(StepFailure::Unverified) => "summary.steps.failure.unverified",
    Some(StepFailure::Unchanged) => "summary.steps.failure.unchanged",
  }
}
