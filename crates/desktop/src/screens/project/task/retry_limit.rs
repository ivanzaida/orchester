//! Retry limit reached (02.B `J3JZaT`): a step kept failing after its failure route was taken as
//! often as it allows, or broke again after its restart, so the run waits for a person. The evidence
//! (`yYFkk`) names the step and shows the latest attempt's steps and what the failing step reported;
//! the panel (`Lk43S`, [`RetryDecisionPanel`]) shows the budget used and the person's decisions.
//! A run that failed after its retries (a person stopped it, or it failed before runs waited) shows
//! the evidence alone. *Runs of this task* (ORC-63) ends the evidence, which scrolls with the scroll
//! gutter. The verification report is not available in this version: its button is drawn
//! unavailable.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  resource::StepKey,
  run::{RunStatus, StepFailure, StepStatus, Stuck},
  time::Timestamp,
  tokens::compact_tokens,
};

use super::{
  super::{ProjectNav, page},
  progress::rows,
  retry_decision::{RetryDecisionPanel, RetryDecisionProps},
  summary::{budget_value, with_reason},
};
use crate::{
  display,
  navigation::route::{Page, TaskPageTab},
  tasks::{
    detail::{RunView, TaskDetail},
    progress::progress,
  },
  theme::{ExtraSpacing, ExtraTypography, gutter_scrollbar},
  ui::{button::ActionButton, icon::Icon, notice::Notice},
};

/// The report button's width (`EdGtS`).
const REPORT_BUTTON_WIDTH: f32 = 216.0;

/// The Summary tab of a run whose `step` used up its retries or broke again: with the person's
/// decisions while the run waits for them (`waiting`, with whether unfinished blocking tasks keep it
/// from going on); `reason` says why its task cannot run again, under the notice, and `runs` is
/// *Runs of this task*, after the evidence.
pub(super) fn retry_limit(
  ctx: &mut Ctx,
  run: &RunView,
  step: &StepKey,
  waiting: Option<(&Stuck, bool)>,
  (reason, runs): (Option<Element>, Option<Element>),
) -> Element {
  let broke = matches!(waiting, Some((Stuck::BrokeAgain(_), _)));
  // Reading the window's width re-renders the page as the window resizes.
  let stacked = ctx.window().logical_width() < crate::theme::RETRY_LIMIT_COLUMNS_MIN_WINDOW;
  let panel = waiting.map(|(stuck, blocked)| {
    let props = decision_props(ctx, run, stuck, (stacked, blocked));
    ctx.mount_keyed::<RetryDecisionPanel>(
      &format!("retry-decision-{}-{}", run.id(), run.run.last_sequence().get()),
      props,
    )
  });
  let evidence = evidence(ctx, run, (step, broke), reason)
    .with_children(runs)
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Section);
  if !stacked {
    // The evidence scrolls beside the panel; its right padding gives the scroll gutter its room.
    let evidence = ScrollVertical::new(evidence.padding_right(ExtraSpacing::BesideGutter.role()))
      .id("task-retry-limit-evidence")
      .flex(1.0)
      .height(Dimension::Pct(100.0))
      .scrollbar(gutter_scrollbar());
    return Row::new()
      .id("task-retry-limit")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .child(evidence)
      .with_children(panel)
      .into();
  }
  // Too narrow for both columns: the panel goes under the evidence, and the page scrolls.
  Column::new()
    .id("task-retry-limit")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .child(
      ScrollVertical::new(
        Column::new()
          .width(Dimension::Pct(100.0))
          .child(evidence)
          .with_children(panel),
      )
      .id("task-retry-limit-page")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .scrollbar(gutter_scrollbar()),
    )
    .into()
}

/// *Open conversation* in the top bar of a run that waits for a person, before *Cancel run*
/// (`A3JNZ` in 02.B3): the task's details are not edited while it waits.
pub(super) fn open_conversation(ctx: &Ctx, nav: &ProjectNav, detail: &TaskDetail) -> Option<Element> {
  let run = detail.run.as_ref()?;
  if !matches!(run.run.status(), RunStatus::WaitingForPerson(_)) {
    return None;
  }
  Some(
    ActionButton::new(ctx.t("runs.ready.openConversation"))
      .icon(Icon::MessagesSquare)
      .id("task-open-conversation")
      .height(Dimension::Px(page::TOPBAR_BUTTON_HEIGHT))
      .link(nav.link(Page::Task(detail.task.id(), TaskPageTab::Conversation)))
      .render(ctx),
  )
}

/// What the decision panel shows of the waiting run.
fn decision_props(ctx: &Ctx, run: &RunView, stuck: &Stuck, (stacked, blocked): (bool, bool)) -> RetryDecisionProps {
  let failed = stuck.step();
  let retry_from = run.run.retry_from().unwrap_or(failed).clone();
  let left = match run.run.budget() {
    Some(budget) => ctx.t_args(
      "runs.retryLimit.remainingValue",
      [("count", compact_tokens(run.run.used().left_of(budget)).as_str())],
    ),
    None => ctx.t("tokens.noLimit"),
  };
  let targets: Arc<[(StepKey, Arc<str>)]> = run
    .run
    .return_targets()
    .into_iter()
    .map(|key| {
      let name = Arc::from(step_name(run, &key));
      (key, name)
    })
    .collect();
  RetryDecisionProps {
    run: run.id(),
    stuck: stuck.clone(),
    used: budget_value(ctx, run),
    left,
    retry_from_name: Arc::from(step_name(run, &retry_from)),
    retry_from,
    failed: Arc::from(step_name(run, failed)),
    targets,
    stacked,
    blocked,
  }
}

fn step_name<'a>(run: &'a RunView, step: &'a StepKey) -> &'a str {
  run
    .steps
    .iter()
    .find(|view| view.key == *step)
    .map_or(step.as_str(), |view| &view.name)
}

/// What failed and the latest attempt's steps (`yYFkk`); `broke`: the step broke again rather than
/// using up its retries.
fn evidence(ctx: &Ctx, run: &RunView, (step, broke): (&StepKey, bool), reason: Option<Element>) -> Column {
  let taken = run.run.automatic_retries_taken(step);
  let title = match (broke, taken) {
    (true, _) => "runs.retryLimit.brokeTitle",
    (false, 1) => "runs.retryLimit.title.one",
    (false, _) => "runs.retryLimit.title.other",
  };
  let detail = if broke {
    "runs.retryLimit.brokeDetail"
  } else {
    "runs.retryLimit.detail"
  };
  let retries = taken.to_string();
  let name = step_name(run, step);
  let notice = Notice::new(
    Icon::AlertCircle,
    ctx.t_args(title, [("step", name), ("retries", retries.as_str())]),
    ctx.t(detail),
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
  let now = Timestamp::now();
  let attempt_rows: Vec<_> = progress(run, false, now)
    .into_iter()
    .filter(|row| executed.contains(&&*row.id))
    .collect();
  Column::new()
    .spacing(ExtraSpacing::Form.role())
    .child(with_reason(notice.into(), reason))
    .child(Text::new(&latest).variant(ExtraTypography::Subheading.role()))
    .child(rows(ctx, &attempt_rows, ExtraSpacing::Form.role(), now))
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
          .width(Dimension::Pct(100.0))
          .variant(ExtraTypography::MonoField.role())
          .color(PaletteColor::TextSecondary)
          .selectable(true),
      )
      .into(),
  )
}
