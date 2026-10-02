//! Run details of a run ready for review (`cB4GN`): the project, the pinned pipeline, the revision
//! the run started from and its working branch (`pN4LP`, `UcSVd`), its active agent time and
//! tokens, and the agents that worked on it (`it5nz`, `mBkeJ` rows). The run records the revision
//! it started from, not a branch, so the base reads as that revision. The instruction snapshots
//! row belongs to instructions and is left out; Open conversation opens the task conversation.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text, TextOverflow},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{integration::Leftovers, time::Timestamp};

use super::super::{activity::engine, summary::budget_value};
use crate::{
  display,
  messages::Message,
  tasks::detail::{RunView, StepView},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    icon::{Icon, IconSize, icon},
    link::Link,
    message_text,
    property::{ValueStyle, property, property_with_suffix, styled_property},
  },
};

/// Panel width (`cB4GN`), an agent row's height and its glyph tile (`mBkeJ` instances in `it5nz`).
const PANEL_WIDTH: f32 = 300.0;
const AGENT_HEIGHT: f32 = 44.0;
const AGENT_GLYPH: f32 = 28.0;

pub(super) fn run_details(
  ctx: &Ctx,
  run: &RunView,
  project: &str,
  leftovers: Option<&Result<Leftovers, Message>>,
  merged: bool,
  open: Link,
) -> Element {
  let version = run.version.to_string();
  let pipeline = ctx.t_args(
    "runs.summary.pipelineValue",
    [("name", &*run.pipeline), ("version", &version)],
  );
  let duration = message_text(ctx, &display::duration(run.active_time(Timestamp::now())));
  let properties = Column::new()
    .id("task-ready-details")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Property.role())
    .child(property(&ctx.t("runs.ready.project"), project))
    .child(property(&ctx.t("runs.summary.pipeline"), &pipeline))
    .child(styled_property(
      &ctx.t("runs.ready.base"),
      display::short_revision(run.run.base().as_str()),
      ValueStyle::Mono,
    ))
    .child(working_branch_property(ctx, run, leftovers, merged))
    .child(styled_property(
      &ctx.t("runs.ready.duration"),
      &duration,
      ValueStyle::Mono,
    ))
    .child(styled_property(
      &ctx.t("runs.ready.tokens"),
      &budget_value(ctx, run),
      ValueStyle::Mono,
    ));
  Column::new()
    .id("task-run-details")
    .width(Dimension::Px(PANEL_WIDTH))
    .height(Dimension::Pct(100.0))
    .padding(SpacingSize::Lg)
    .spacing(ExtraSpacing::Step.role())
    .background(PaletteColor::SurfacePanel)
    .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(&ctx.t("runs.ready.details")).variant(TypographyStyle::Label))
    .child(properties)
    .with_children(agents(ctx, run))
    .child(Spacer::new().flex(1.0))
    .child(
      ActionButton::new(ctx.t("runs.ready.openConversation"))
        .icon(Icon::MessagesSquare)
        .id("task-open-conversation")
        .fill_width()
        .link(open)
        .render(ctx),
    )
    .into()
}

fn working_branch_property(
  ctx: &Ctx,
  run: &RunView,
  leftovers: Option<&Result<Leftovers, Message>>,
  merged: bool,
) -> Element {
  let label = ctx.t("runs.ready.workingBranch");
  match leftovers {
    Some(Ok(leftovers))
      if leftovers
        .branches
        .iter()
        .all(|branch| branch.name.as_str() != run.run.branch().as_str()) =>
    {
      property_with_suffix(
        &label,
        run.run.branch().as_str(),
        &ctx.t("runs.ready.workingBranchDeleted"),
      )
    }
    Some(Err(_)) => property(&label, &ctx.t("runs.ready.workingBranchUnavailable")),
    None if merged => property(&label, &ctx.t("runs.ready.workingBranchChecking")),
    _ => property(&label, run.run.branch().as_str()),
  }
}

/// The agents that worked on the run, each with its CLI, model and steps (`it5nz`).
fn agents(ctx: &Ctx, run: &RunView) -> Option<Element> {
  let mut agents: Vec<(Arc<str>, Arc<str>, Vec<&str>)> = Vec::new();
  // A tool step's command is no agent.
  for step in run
    .steps
    .iter()
    .filter(|step| step.execution.is_some() && !step.tool_step)
  {
    let Some(agent) = step.agent.clone() else {
      continue;
    };
    let on = on_engine(ctx, step);
    match agents
      .iter_mut()
      .find(|(role, engine, _)| *role == agent && *engine == on)
    {
      Some((_, _, steps)) => steps.push(&step.name),
      None => agents.push((agent, on, vec![&step.name])),
    }
  }
  if agents.is_empty() {
    return None;
  }
  let separator = ctx.t("common.listSeparator");
  let rows = agents.into_iter().map(|(role, on, steps)| {
    let detail = ctx.t_args(
      "runs.ready.agentDetail",
      [("engine", &*on), ("steps", steps.join(&separator).as_str())],
    );
    agent_row(&role, &detail)
  });
  Some(
    Column::new()
      .id("task-ready-agents")
      .width(Dimension::Pct(100.0))
      .padding_top(ExtraSpacing::Divider.role())
      .spacing(ExtraSpacing::Stack.role())
      .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(Text::new(&ctx.t("runs.ready.agents")).variant(ExtraTypography::FormLabel.role()))
      .with_children(rows)
      .into(),
  )
}

/// "Codex / gpt-5.5", or the model alone when the step's connection is gone.
fn on_engine(ctx: &Ctx, step: &StepView) -> Arc<str> {
  engine(ctx, step.provider, step.model.as_deref().unwrap_or_default())
}

/// One agent (`mBkeJ`): its glyph, its role and what it ran on.
fn agent_row(role: &str, detail: &str) -> Element {
  let tile = Row::new()
    .size(Dimension::Px(AGENT_GLYPH), Dimension::Px(AGENT_GLYPH))
    .background(PaletteColor::SurfaceRaised)
    .rounded(RadiusSize::Md)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(icon(Icon::Bot, IconSize::Event, PaletteColor::TextSecondary));
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(AGENT_HEIGHT))
    .spacing(ExtraSpacing::Stack.role())
    .child(tile)
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(role)
            .variant(TypographyStyle::Caption)
            .color(PaletteColor::TextPrimary)
            .nowrap(),
        )
        .child(
          Text::new(detail)
            .variant(ExtraTypography::Footnote.role())
            .nowrap()
            .text_overflow(TextOverflow::Elipsis),
        ),
    )
    .into()
}
