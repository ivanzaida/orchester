//! The run's steps (`Step progress` `BUVJ9` in `lUXff`): a progress row per step (`Step progress row`
//! `AfpbP`), in stage order, parallel steps that have not started included: its status icon, its
//! number, name and agent, and at the end where it stands with its attempt, the tokens it used and its
//! active agent time ("Complete · attempt 1 · 41k tokens · 6 min"). A step whose agent waits for the
//! person's approval says so and since when.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{run::StepFailure, time::Timestamp, tokens::compact_tokens};
use orchester_workspace::{
  display,
  tasks::{
    detail::RunView,
    progress::{ProgressRow, ProgressState, progress},
  },
};

use super::{live::Ticker, section_head};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize, TaskText},
  ui::{
    icon::{Icon, icon},
    message::message_text,
    status::Status,
  },
};

/// A progress row's height (`AfpbP`).
const ROW_HEIGHT: f32 = 40.0;

/// One step's row: its number in the pipeline, its progress, and since when its agent waits for the
/// person's approval.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepRow {
  number: usize,
  progress: ProgressRow,
  waits_since: Option<Timestamp>,
}

/// *Steps* with its count and the rows of `run`'s steps at `now`. `lost`: the run is recorded as running
/// but has no runner; `waiting`: the steps, by key, whose agent waits for an approval, and since when.
pub fn section(ctx: &mut Ctx, run: &RunView, lost: bool, waiting: &[(Arc<str>, Timestamp)], now: Timestamp) -> Element {
  let rows = rows(run, lost, waiting, now);
  Column::new()
    .id("task-steps")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .child(section_head(
      ctx,
      "task-steps-head",
      "summary.steps.title",
      Some(rows.len()),
    ))
    .child(ctx.mount_keyed::<LiveSteps>("task-steps-rows", StepsProps { rows }))
    .into()
}

/// The rows of `run`'s steps at `now`, in stage order; `lost` and `waiting` as for [`section`].
pub(super) fn rows(run: &RunView, lost: bool, waiting: &[(Arc<str>, Timestamp)], now: Timestamp) -> Vec<StepRow> {
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

impl StepRow {
  /// The key of the row's step.
  pub(super) fn key(&self) -> &str {
    &self.progress.id
  }
}

#[derive(Clone, PartialEq)]
struct StepsProps {
  rows: Vec<StepRow>,
}

impl lurq::app::component::DevtoolsInspectable for StepsProps {}

/// The rows, their active agent time going on while their agents work.
struct LiveSteps {
  ticker: Ticker,
}

impl Component for LiveSteps {
  type Props = StepsProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      ticker: Ticker::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let rows = ctx.props::<StepsProps>().rows.clone();
    let counting = rows
      .iter()
      .any(|row| row.progress.usage.is_some_and(|usage| usage.active.counting()));
    let now = self.ticker.now(ctx, counting);
    Column::new()
      .id("task-step-rows")
      .width(Dimension::Pct(100.0))
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .clip()
      .with_children(
        rows
          .iter()
          .enumerate()
          .map(|(index, row)| step_row(ctx, row, index > 0, now)),
      )
  }
}

pub(super) fn step_row(ctx: &Ctx, row: &StepRow, divided: bool, now: Timestamp) -> Element {
  let progress = &row.progress;
  let (glyph, color) = match (row.waits_since, unjudged(progress.state)) {
    (None, Some(glyph)) => (glyph, PaletteColor::Warning),
    (Some(_), _) => (Status::NeedsInput.icon(), Status::NeedsInput.color()),
    (None, None) => (status(progress.state).icon(), status(progress.state).color()),
  };
  let line = Row::new()
    .id(format!("task-step-{}", progress.id))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Inset.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .describe("step", progress.id.to_string())
    .describe("state", format!("{:?}", progress.state))
    .child(icon(glyph, IconSize::Md, color))
    .child(Text::new(&format!("{:02}", row.number)).variant(ExtraTypography::MonoKey.role()))
    .child(Text::new(&progress.step).variant(TaskText::StepName.role()).nowrap())
    .with_children(progress.agent.as_deref().map(|agent| {
      Text::new(agent)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap()
    }))
    .child(Spacer::new().flex(1.0))
    .child(
      Text::new(&detail(ctx, row, now))
        .id(format!("task-step-{}-state", progress.id))
        .variant(TypographyStyle::Description)
        .color(detail_color(progress.state))
        .nowrap(),
    );
  if divided {
    line
      .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .into()
  } else {
    line.into()
  }
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
/// everything (`circle-dashed`) or was not run again on an unchanged revision (`hand`), as the notice says it.
const fn unjudged(state: ProgressState) -> Option<Icon> {
  match state {
    ProgressState::Failed(Some(StepFailure::Unverified)) => Some(Icon::CircleDashed),
    ProgressState::Failed(Some(StepFailure::Unchanged)) => Some(Icon::Hand),
    _ => None,
  }
}

/// A step at work reads in `text_primary`, one that has not started in `text_muted`, a failed one in
/// `danger`, one that ended without a verdict ([`unjudged`]) in `warning`.
const fn detail_color(state: ProgressState) -> PaletteColor {
  match state {
    ProgressState::Working => PaletteColor::TextPrimary,
    ProgressState::Waiting => PaletteColor::TextMuted,
    ProgressState::Failed(Some(StepFailure::Unverified | StepFailure::Unchanged)) => PaletteColor::Warning,
    ProgressState::Failed(_) => PaletteColor::Danger,
    _ => PaletteColor::TextSecondary,
  }
}

/// Where a step stands, with its attempt, tokens and active agent time at `now` when it ran.
fn detail(ctx: &Ctx, row: &StepRow, now: Timestamp) -> Arc<str> {
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
