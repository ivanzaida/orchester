//! What the decision rests on (`Evidence` `oXmm0`, `What Review reported` `OjW9a`): the latest attempt's
//! steps with the commit its work ended at, what the failing step reported (`Report` `cLh0O`: who, the
//! step, its attempt and time, and the words), and that the run's changes and messages are kept, with
//! links to them (`Preserved` `v133z`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  resource::StepKey,
  run::{StepFailure, StepStatus},
  time::Timestamp,
};
use orchester_workspace::{display, tasks::detail::RunView};

use super::super::{
  super::{TaskContext, steps},
  step_label, tab_link,
};
use crate::{
  routes::TaskTab,
  theme::{DecisionText, ExtraSpacing, IconSize, TaskText},
  ui::{
    card::{CardSurface, card},
    icon::{Icon, icon},
  },
};

/// The reporter's tile (`Avatar` `y8dMj3`).
const AVATAR: f32 = 22.0;

pub fn sections(ctx: &mut Ctx, props: &TaskContext, run: &RunView, failed: &StepKey, now: Timestamp) -> Vec<Element> {
  let mut parts = vec![latest_attempt(ctx, run, now)];
  let report = report(ctx, run, failed);
  let preserved = preserved(ctx, props, run);
  parts.push(
    Column::new()
      .id("task-retry-evidence")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role())
      .with_children(report.into_iter().flatten())
      .child(preserved)
      .into(),
  );
  parts
}

/// *Latest attempt* with the commit its work ended at, and its steps that ran (`Attempt steps` `Nogci`).
fn latest_attempt(ctx: &mut Ctx, run: &RunView, now: Timestamp) -> Element {
  let ran = |key: &str| {
    run.steps.iter().any(|view| {
      let current = view.execution.is_some_and(|execution| {
        run
          .run
          .current_attempt()
          .is_some_and(|attempt| attempt.execution(execution).is_some())
      });
      view.key.as_str() == key && current && matches!(view.status, Some(StepStatus::Passed | StepStatus::Failed))
    })
  };
  let rows: Vec<steps::StepRow> = steps::rows(run, false, &[], now)
    .into_iter()
    .filter(|row| ran(row.key()))
    .collect();
  let revision = run
    .run
    .last_checkpoint()
    .map(|checkpoint| display::short_revision(checkpoint.revision().as_str()).to_owned());
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t("decisions.retries.latestAttempt")).variant(TaskText::SectionTitle.role()))
    .child(Spacer::new().flex(1.0))
    .with_children(revision.map(|revision| {
      Text::new(&revision)
        .id("task-retry-revision")
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextSecondary)
    }));
  let list = Column::new()
    .id("task-retry-attempt")
    .width(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .clip()
    .with_children(
      rows
        .iter()
        .enumerate()
        .map(|(index, row)| steps::step_row(ctx, row, index > 0, now)),
    );
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .child(head)
    .child(list)
    .into()
}

/// *What Review reported* and the report, when the failing step left one.
fn report(ctx: &Ctx, run: &RunView, failed: &StepKey) -> Option<[Element; 2]> {
  let step = run.steps.iter().find(|step| step.key == *failed)?;
  let output = run.output(step.execution?)?;
  if output.text.trim().is_empty() {
    return None;
  }
  let label = step_label(ctx, step);
  let attempt = step.attempt.map(|attempt| attempt.to_string()).unwrap_or_default();
  let time = display::clock(output.created_at);
  let reporter = step.agent.as_deref().unwrap_or(&step.name);
  let glyph = if matches!(step.failure, Some(StepFailure::CommandFailed)) || step.tool_step {
    Icon::Terminal
  } else {
    Icon::ScanEye
  };
  let who = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Row::new()
        .width(Dimension::Px(AVATAR))
        .height(Dimension::Px(AVATAR))
        .align_items(Alignment::Center)
        .justify(Justify::Center)
        .background(PaletteColor::SurfaceRaised)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .rounded(RadiusSize::Md)
        .child(icon(glyph, IconSize::Sm, PaletteColor::TextSecondary)),
    )
    .child(Text::new(reporter).variant(DecisionText::Subhead.role()))
    .child(
      Text::new(&ctx.t_args(
        "decisions.retries.reportedAt",
        [
          ("step", &*label),
          ("attempt", attempt.as_str()),
          ("time", time.as_str()),
        ],
      ))
      .variant(TypographyStyle::Caption),
    );
  let head = Text::new(&ctx.t_args("decisions.retries.reported", [("step", &*step.name)]))
    .id("task-retry-reported-head")
    .variant(TaskText::SectionTitle.role());
  let body = card("task-retry-report", CardSurface::Raised)
    .spacing(SpacingSize::Sm)
    .child(who)
    .child(
      Text::new(&output.text)
        .id("task-retry-report-text")
        .variant(DecisionText::Report.role())
        .width(Dimension::Pct(100.0))
        .selectable(true),
    );
  Some([head.into(), body.into()])
}

/// "Changes and messages are preserved on kon-12-webhook-retry", with Changes ↗ and Conversation ↗.
fn preserved(ctx: &mut Ctx, props: &TaskContext, run: &RunView) -> Element {
  let text = ctx.t_args("decisions.retries.preserved", [("branch", run.run.branch().as_str())]);
  Row::new()
    .id("task-retry-preserved")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::Archive, IconSize::Base, PaletteColor::TextMuted))
    .child(Text::new(&text).variant(TypographyStyle::Description).flex(1.0))
    .child(tab_link(
      ctx,
      props,
      ("task-retry-open-changes", "decisions.openChanges"),
      TaskTab::Changes,
    ))
    .child(tab_link(
      ctx,
      props,
      ("task-retry-open-conversation", "decisions.openConversation"),
      TaskTab::Conversation,
    ))
    .into()
}
