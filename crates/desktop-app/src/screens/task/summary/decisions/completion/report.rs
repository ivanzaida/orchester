//! What a run that committed nothing delivered (ORC-93), under its card: *Report*, the last step's report in Markdown
//! (kept as the run's artifact *Report* once the run was completed), and for work done outside the repository *Changes
//! outside the repository*, each MCP tool call the run was let make: the server and tool, what it acted on, when it was
//! approved and by whom. They follow the Summary's sections (`Head · Steps` `P4Q0cn`, the conflicting files' list
//! `IBx7h`); the design has no frame of its own for them.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  resource::Deliverable,
  run::{DecidedBy, tool_report_markdown},
};
use orchester_workspace::{
  display,
  tasks::{
    completion::{OutsideChange, RunResult},
    detail::RunView,
  },
};

use super::super::super::section_head;
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    card::{CardSurface, card},
    chat::links,
    icon::{Icon, icon},
    markdown::MarkdownView,
  },
};

/// *Report* and, for work done outside the repository, *Changes outside the repository*.
pub fn sections(ctx: &mut Ctx, run: &RunView, result: &RunResult) -> Vec<Element> {
  let report = report(ctx, run, result);
  let outside = (result.delivers == Deliverable::Outside).then(|| outside(ctx, &result.outside));
  report.into_iter().chain(outside).collect()
}

/// The run's report, with the step that wrote it; *No report* when no step reported anything.
fn report(ctx: &mut Ctx, run: &RunView, result: &RunResult) -> Option<Element> {
  let body: Element = match &result.report {
    Some(report) => {
      let view = report
        .step
        .as_ref()
        .and_then(|key| run.steps.iter().find(|step| step.key == *key));
      let step = view.map(|step| super::super::step_label(ctx, step));
      // A tool step's program output is not Markdown.
      let text = if view.is_some_and(|step| step.tool_step) {
        tool_report_markdown(&report.text)
      } else {
        report.text.to_string()
      };
      let text = MarkdownView::new("task-report-text", text, links(ctx))
        .style(ExtraTypography::Lead.role(), PaletteColor::TextSecondary)
        .render(ctx);
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .with_children(step.map(|step| {
          Text::new(&ctx.t_args("summary.report.from", [("step", &*step)]))
            .id("task-report-step")
            .variant(ExtraTypography::Note.role())
        }))
        .child(text)
        .into()
    }
    None => Text::new(&ctx.t("summary.report.none"))
      .id("task-report-none")
      .variant(ExtraTypography::Note.role())
      .into(),
  };
  let frame = card("task-report-card", CardSurface::Outline)
    .padding(SpacingSize::Lg)
    .child(body);
  Some(
    Column::new()
      .id("task-report")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role())
      .child(section_head(ctx, "task-report-head", "summary.report.title", None))
      .child(frame)
      .into(),
  )
}

/// *Changes outside the repository*, a row per MCP tool call; *None recorded* when the run was let make none.
fn outside(ctx: &Ctx, changes: &[OutsideChange]) -> Element {
  let list: Element = if changes.is_empty() {
    Text::new(&ctx.t("summary.outside.none"))
      .id("task-outside-none")
      .variant(ExtraTypography::Note.role())
      .into()
  } else {
    card("task-outside-list", CardSurface::Outline)
      .padding(Padding::symmetric(ExtraSpacing::Snug.role(), ExtraSpacing::Hair.role()))
      .with_children(
        changes
          .iter()
          .enumerate()
          .map(|(index, change)| row(ctx, index, change)),
      )
      .into()
  };
  Column::new()
    .id("task-outside")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .child(section_head(
      ctx,
      "task-outside-head",
      "summary.outside.title",
      Some(changes.len()),
    ))
    .child(list)
    .into()
}

/// One tool call: *pencil · batch_design*, what it acted on, and *approved by you · 14:02*.
fn row(ctx: &Ctx, index: usize, change: &OutsideChange) -> Element {
  let when = ctx.t_args(approved_by(change.by), [("time", display::clock(change.at))]);
  let call = ctx.t_args(
    "summary.outside.call",
    [("server", &*change.server), ("tool", &*change.tool)],
  );
  Row::new()
    .id(format!("task-outside-{index}"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(icon(Icon::Plug, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Fine.role())
        .child(Text::new(&call).variant(TypographyStyle::Mono))
        .with_children(change.target.as_ref().map(|target: &Arc<str>| {
          Text::new(target)
            .variant(ExtraTypography::Note.role())
            .width(Dimension::Pct(100.0))
        })),
    )
    .child(Text::new(&when).variant(TypographyStyle::Caption).nowrap())
    .into()
}

/// The words of who approved a tool call, by `by`.
const fn approved_by(by: Option<DecidedBy>) -> &'static str {
  match by {
    Some(DecidedBy::Person | DecidedBy::PersonSimilar | DecidedBy::PersonAlways | DecidedBy::PersonRun) => {
      "summary.outside.byYou"
    }
    Some(DecidedBy::SimilarRule | DecidedBy::AlwaysAllowed) => "summary.outside.byRule",
    Some(DecidedBy::AutoApproved | DecidedBy::Permissions) => "summary.outside.automatically",
    Some(DecidedBy::Automation) => "summary.outside.byAutomation",
    Some(DecidedBy::Reviewer) => "summary.outside.byReviewer",
    Some(DecidedBy::PersonDeny | DecidedBy::AutomationDeny) | None => "summary.outside.approved",
  }
}
