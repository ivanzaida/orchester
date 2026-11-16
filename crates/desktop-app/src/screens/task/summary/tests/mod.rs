//! The Tests card on a task's Summary (ORC-266, `07 · Summary · Tests` `qN2FK` in `Agent testing · redesign`
//! `eWydp`): what the run's latest Test step found, from the record it kept in the run's storage
//! (`orchester_workspace::tasks::test_results`). It takes the place of that step's report among the run's output. In a
//! `border` outline, radius 10: its head (`va09D`), 12 over and under and 14 at its sides, 8 apart — a 14
//! `flask-conical` in `text_secondary`, *Tests* in 13/600, the step, its attempt, when it ended and how long it took
//! in 12 `text_muted`, and at its end how many tests failed (`danger`) and were flaky (`warning`) as badges; then a
//! row per layer ([`layers`]), what failed and was flaky ([`failed`]) and the files the suites left
//! ([`artifacts`]). A run without a Test step that ended has no card.

mod artifacts;
mod failed;
mod layers;
pub mod returned;
mod words;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::test_step::TestStepRecord;
use orchester_workspace::{
  display,
  tasks::{detail::RunView, test_results::TestResults},
};

use crate::{
  theme::{ExtraSpacing, IconSize, TaskText},
  ui::{
    chip::{BadgeTone, badge},
    icon::{Icon, icon},
  },
};

/// The card of `tests`, what `run`'s latest Test step found.
pub fn card(ctx: &mut Ctx, run: &RunView, tests: &TestResults) -> Element {
  let card = Column::new()
    .id("task-tests")
    .width(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg);
  let record = match &tests.record {
    Ok(record) => record,
    Err(reason) => {
      let words = ctx.t_args("summary.tests.unreadable", [("reason", &**reason)]);
      let why = Text::new(&words)
        .id("task-tests-unreadable")
        .variant(TypographyStyle::Description)
        .color(PaletteColor::Danger)
        .width(Dimension::Pct(100.0))
        .padding(Padding::symmetric(
          ExtraSpacing::Inline.role(),
          ExtraSpacing::Inset.role(),
        ))
        .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border));
      return card.child(head(ctx, run, tests, None)).child(why).into();
    }
  };
  let returned_to = tests.returned_to.as_ref().map(|step| step_label(run, step.as_str()));
  card
    .child(head(ctx, run, tests, Some(record)))
    .with_children(record.layers.iter().map(|layer| layers::row(ctx, layer)))
    .with_children(failed::section(ctx, record, returned_to.as_deref()))
    .with_children(failed::flaky_section(ctx, record))
    .with_children(artifacts::section(ctx, record))
    .into()
}

/// A step as the run's rail numbers it, `04 Test`; its key alone once the run's pipeline no longer has it.
pub fn step_label(run: &RunView, key: &str) -> String {
  run
    .steps
    .iter()
    .find(|step| step.key.as_str() == key)
    .map_or_else(|| key.to_owned(), |step| format!("{:02} {}", step.number, step.name))
}

/// The card's head: *Tests*, which execution it shows, and how many tests failed and were flaky.
fn head(ctx: &Ctx, run: &RunView, tests: &TestResults, record: Option<&TestStepRecord>) -> Element {
  let step = step_label(run, tests.step.as_str());
  let attempt = tests.attempt.get().to_string();
  let ended = run
    .invocations_of(tests.execution)
    .last()
    .and_then(|invocation| Some((invocation.started_at, invocation.ended_at?)));
  let meta = match ended {
    Some((started, ended)) => {
      // Only a clock that went backwards ends a step before it started: it took no time then.
      let took = u64::try_from(ended.unix_millis() - started.unix_millis()).unwrap_or(0);
      ctx.t_args(
        "summary.tests.meta",
        [
          ("step", step),
          ("attempt", attempt),
          ("time", display::clock(ended)),
          ("duration", words::duration(ctx, took).to_string()),
        ],
      )
    }
    None => ctx.t_args("summary.tests.metaShort", [("step", step), ("attempt", attempt)]),
  };
  Row::new()
    .id("task-tests-head")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Inset.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::FlaskConical, IconSize::Md, PaletteColor::TextSecondary))
    .child(
      Text::new(&ctx.t("summary.tests.title"))
        .id("task-tests-title")
        .variant(TaskText::SectionTitle.role())
        .nowrap(),
    )
    .child(
      Text::new(&meta)
        .id("task-tests-meta")
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .min_width(Dimension::Px(0.0))
        .nowrap(),
    )
    .child(Row::new().flex(1.0))
    .with_children(record.map(|record| badges(ctx, record)).unwrap_or_default())
    .into()
}

/// How many tests failed and were flaky, or that every test passed.
fn badges(ctx: &Ctx, record: &TestStepRecord) -> Vec<Element> {
  let failed = record.failed().count();
  let flaky = record.flaky().count();
  let passed = record.layers.iter().all(|layer| layer.passed());
  let mut badges = Vec::new();
  if failed > 0 {
    let word = ctx.t_args("summary.tests.failed", [("count", failed.to_string())]);
    badges.push(badge("task-tests-failed-badge", BadgeTone::Danger, &word).into());
  }
  if flaky > 0 {
    let word = ctx.t_args("summary.tests.flaky", [("count", flaky.to_string())]);
    badges.push(badge("task-tests-flaky-badge", BadgeTone::Warning, &word).into());
  }
  if passed && failed == 0 {
    badges.push(
      badge(
        "task-tests-passed-badge",
        BadgeTone::Success,
        &ctx.t("summary.tests.allPassed"),
      )
      .into(),
    );
  }
  badges
}
