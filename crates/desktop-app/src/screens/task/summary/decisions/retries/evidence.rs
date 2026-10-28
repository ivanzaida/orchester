//! What the decision rests on (`Evidence` `oXmm0`, `What Review reported` `OjW9a`): what the failing step reported
//! (`Report` `cLh0O`: who, the step, its attempt and time, and the words, read as Markdown with the Conversation's
//! `ui::markdown`), and that the run's changes and messages are kept, with links to them (`Preserved` `v133z`). The
//! latest attempt's steps are the Summary's rail (ORC-100).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  resource::StepKey,
  run::{StepFailure, tool_report_markdown},
};
use orchester_workspace::{display, tasks::detail::RunView};

use super::super::{super::TaskContext, step_label, tab_link};
use crate::{
  routes::TaskTab,
  theme::{DecisionText, ExtraSpacing, IconSize, TaskText},
  ui::{
    card::{CardSurface, card},
    chat::links,
    icon::{Icon, icon},
    markdown::MarkdownView,
  },
};

/// The reporter's tile (`Avatar` `y8dMj3`).
const AVATAR: f32 = 22.0;

pub fn sections(ctx: &mut Ctx, props: &TaskContext, run: &RunView, failed: &StepKey) -> Vec<Element> {
  let report = report(ctx, run, failed);
  let preserved = preserved(ctx, props, run);
  vec![
    Column::new()
      .id("task-retry-evidence")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role())
      .with_children(report.into_iter().flatten())
      .child(preserved)
      .into(),
  ]
}

/// *What Review reported* and the report, when the failing step left one.
fn report(ctx: &mut Ctx, run: &RunView, failed: &StepKey) -> Option<[Element; 2]> {
  let step = run.steps.iter().find(|step| step.key == *failed)?;
  // A step not run again on an unchanged revision left no output: its earlier result is what it reported.
  let output = step
    .execution
    .and_then(|execution| run.output(execution))
    .or_else(|| run.latest_output(failed))?;
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
  // What an agent wrote is Markdown, as the Conversation reads it; a tool step's program output is not.
  let text: Arc<str> = if step.tool_step {
    tool_report_markdown(&output.text).into()
  } else {
    output.text.as_str().into()
  };
  let text = MarkdownView::new("task-retry-report-text", text, links(ctx))
    .style(DecisionText::Report.role(), PaletteColor::TextPrimary)
    .render(ctx);
  let body = card("task-retry-report", CardSurface::Raised)
    .spacing(SpacingSize::Sm)
    .child(who)
    .child(text);
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
