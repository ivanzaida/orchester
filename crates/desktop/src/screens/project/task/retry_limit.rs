//! Retry limit reached (02.B `J3JZaT`): a step kept failing after its failure route was taken as
//! often as it allows, so the run failed. The evidence (`yYFkk`) names the step and shows the latest
//! attempt's steps and what the failing step reported; the panel (`Lk43S`) shows the budget used
//! and what another attempt would need. Allowing another attempt, the verification report and
//! revising the plan are not available in this version: they are drawn unavailable and say so.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  resource::StepKey,
  run::{StepFailure, StepStatus},
  time::Timestamp,
  tokens::compact_tokens,
};

use super::{progress::rows, summary::budget_value};
use crate::{
  display,
  tasks::{detail::RunView, progress::progress},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::{ValueTone, value_box},
    icon::Icon,
    notice::Notice,
    property::property,
  },
};

/// Settings panel width (`Lk43S`) and the report button's width (`EdGtS`).
const PANEL_WIDTH: f32 = 360.0;
const REPORT_BUTTON_WIDTH: f32 = 216.0;

/// The Summary tab of a run whose `step` used up its retries.
pub(super) fn retry_limit(ctx: &Ctx, run: &RunView, step: &StepKey) -> Element {
  Row::new()
    .id("task-retry-limit")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .child(evidence(ctx, run, step))
    .child(settings(ctx, run, step))
    .into()
}

fn step_name<'a>(run: &'a RunView, step: &'a StepKey) -> &'a str {
  run
    .steps
    .iter()
    .find(|view| view.key == *step)
    .map_or(step.as_str(), |view| &view.name)
}

/// What failed and the latest attempt's steps (`yYFkk`).
fn evidence(ctx: &Ctx, run: &RunView, step: &StepKey) -> Element {
  let taken = run.run.retries_taken(step);
  let title = if taken == 1 {
    "runs.retryLimit.title.one"
  } else {
    "runs.retryLimit.title.other"
  };
  let retries = taken.to_string();
  let name = step_name(run, step);
  let notice = Notice::new(
    Icon::AlertCircle,
    ctx.t_args(title, [("step", name), ("retries", retries.as_str())]),
    ctx.t("runs.retryLimit.detail"),
  )
  .id("task-notice");
  let attempt = run
    .run
    .current_attempt()
    .map_or_else(String::new, |attempt| attempt.number().to_string());
  let latest = match run.run.last_checkpoint() {
    Some(checkpoint) => ctx.t_args(
      "runs.retryLimit.latest",
      [("revision", display::short_revision(checkpoint.revision().as_str()))],
    ),
    None => ctx.t_args(
      "runs.retryLimit.latestWithoutCheckpoint",
      [("attempt", attempt.as_str())],
    ),
  };
  let executed: Vec<&str> = run
    .steps
    .iter()
    .filter(|view| {
      let current = view.execution.is_some_and(|execution| {
        run
          .run
          .current_attempt()
          .is_some_and(|attempt| attempt.execution(execution).is_some())
      });
      current && matches!(view.status, Some(StepStatus::Passed | StepStatus::Failed))
    })
    .map(|view| view.key.as_str())
    .collect();
  let attempt_rows: Vec<_> = progress(run, false, Timestamp::now())
    .into_iter()
    .filter(|row| executed.contains(&&*row.id))
    .collect();
  Column::new()
    .flex(1.0)
    .height(Dimension::Pct(100.0))
    .padding(SpacingSize::Section)
    .spacing(ExtraSpacing::Form.role())
    .child(notice)
    .child(Text::new(&latest).variant(ExtraTypography::Subheading.role()))
    .child(rows(ctx, &attempt_rows))
    .with_children(excerpt(ctx, run, step))
    .child(
      ActionButton::new(ctx.t("runs.retryLimit.openReport"))
        .icon(Icon::FileText)
        .id("task-open-report")
        .width(Dimension::Px(REPORT_BUTTON_WIDTH))
        .disabled()
        .render(ctx),
    )
    .child(Notice::new(
      Icon::History,
      ctx.t("runs.retryLimit.preservedTitle"),
      ctx.t_args(
        "runs.retryLimit.preservedDetail",
        [("branch", run.run.branch().as_str())],
      ),
    ))
    .into()
}

/// What the failing step's agent reported when it failed (`NMg2D`).
fn excerpt(ctx: &Ctx, run: &RunView, step: &StepKey) -> Option<Element> {
  let view = run.steps.iter().find(|view| view.key == *step)?;
  if !matches!(
    view.failure,
    Some(StepFailure::AgentFailed | StepFailure::ReportedFailed)
  ) {
    return None;
  }
  let output = run.output(view.execution?)?;
  if output.text.is_empty() {
    return None;
  }
  Some(
    Column::new()
      .id("task-failure-excerpt")
      .width(Dimension::Pct(100.0))
      .padding(ExtraSpacing::Step.role())
      .spacing(ExtraSpacing::Stack.role())
      .background(PaletteColor::SurfacePanel)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .child(
        Text::new(&ctx.t_args("runs.retryLimit.reported", [("step", &*view.name)]))
          .variant(TypographyStyle::FieldLabel)
          .color(PaletteColor::TextPrimary),
      )
      .child(
        Text::new(&output.text)
          .variant(ExtraTypography::MonoField.role())
          .color(PaletteColor::TextSecondary)
          .selectable(true),
      )
      .into(),
  )
}

/// Allow another attempt (`Lk43S`): what the run used and has left, and the settings another
/// attempt would take — unavailable in this version, which the panel says.
fn settings(ctx: &Ctx, run: &RunView, step: &StepKey) -> Element {
  let left = ctx.t_args(
    "runs.retryLimit.remainingValue",
    [(
      "count",
      compact_tokens(run.run.used().left_of(run.run.budget())).as_str(),
    )],
  );
  let resume = ctx.t_args("runs.retryLimit.resumeValue", [("step", step_name(run, step))]);
  let field = |label: &str, id: &'static str, value: &str| {
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Field.role())
      .child(Text::new(label).variant(TypographyStyle::Caption))
      .child(value_box(ctx, id, value, ValueTone::Unavailable, None))
  };
  Column::new()
    .id("task-retry-settings")
    .width(Dimension::Px(PANEL_WIDTH))
    .height(Dimension::Pct(100.0))
    .padding(SpacingSize::Lg)
    .spacing(ExtraSpacing::Form.role())
    .background(PaletteColor::SurfacePanel)
    .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(&ctx.t("runs.retryLimit.allowTitle")).variant(TypographyStyle::Title))
    .child(property(&ctx.t("runs.retryLimit.used"), &budget_value(ctx, run)))
    .child(property(&ctx.t("runs.retryLimit.remaining"), &left))
    .child(field(
      &ctx.t("runs.retryLimit.additional"),
      "task-retry-attempts",
      &ctx.t("runs.retryLimit.additionalValue"),
    ))
    .child(field(
      &ctx.t("runs.retryLimit.resumeFrom"),
      "task-retry-resume",
      &resume,
    ))
    .child(Text::new(&ctx.t("runs.retryLimit.unavailable")).variant(ExtraTypography::Explanation.role()))
    .child(Spacer::new().flex(1.0))
    .child(
      ActionButton::new(ctx.t("runs.retryLimit.allow"))
        .icon(Icon::RefreshCw)
        .primary()
        .fill_width()
        .id("task-allow-retry")
        .disabled()
        .render(ctx),
    )
    .child(
      ActionButton::new(ctx.t("runs.retryLimit.revisePlan"))
        .icon(Icon::MessageSquare)
        .fill_width()
        .id("task-revise-plan")
        .disabled()
        .render(ctx),
    )
    .into()
}
