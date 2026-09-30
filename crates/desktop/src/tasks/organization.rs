use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Select, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::task::organization::{Priority, TaskOrganization, WorkflowStatus};

use crate::{
  theme::ExtraSpacing,
  ui::{
    field::select_style,
    icon::{Icon, IconSize, icon},
    status_label::status_label,
  },
};

/// Organization controls in gNExS/Sd5qt/ixENv.
pub const CONTROL_HEIGHT: f32 = 40.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkflowChoice(pub WorkflowStatus);
impl lurq::app::component::DevtoolsInspectable for WorkflowChoice {}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PriorityChoice(pub Priority);
impl lurq::app::component::DevtoolsInspectable for PriorityChoice {}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OrganizationValue(pub TaskOrganization);
impl lurq::app::component::DevtoolsInspectable for OrganizationValue {}

pub const fn workflow_copy(status: WorkflowStatus) -> (Icon, &'static str) {
  match status {
    WorkflowStatus::Backlog => (Icon::Archive, "tasks.workflow.backlog"),
    WorkflowStatus::Todo => (Icon::Circle, "tasks.workflow.todo"),
    WorkflowStatus::InProgress => (Icon::CircleDot, "tasks.workflow.inProgress"),
    WorkflowStatus::InReview => (Icon::CircleCheck, "tasks.workflow.inReview"),
    WorkflowStatus::Done => (Icon::Check, "tasks.workflow.done"),
    WorkflowStatus::Canceled => (Icon::CircleX, "tasks.workflow.canceled"),
  }
}

pub const fn priority_copy(priority: Priority) -> (Icon, &'static str) {
  match priority {
    Priority::Urgent => (Icon::AlertCircle, "tasks.priority.urgent"),
    Priority::High => (Icon::SignalHigh, "tasks.priority.high"),
    Priority::Medium => (Icon::SignalMedium, "tasks.priority.medium"),
    Priority::Low => (Icon::SignalLow, "tasks.priority.low"),
    Priority::None => (Icon::Minus, "tasks.priority.none"),
  }
}

pub fn workflow_label(ctx: &Ctx, status: WorkflowStatus) -> Element {
  let (glyph, key) = workflow_copy(status);
  status_label(glyph, &ctx.t(key))
}

pub fn priority_label(ctx: &Ctx, priority: Priority) -> Element {
  let (glyph, key) = priority_copy(priority);
  Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Small, PaletteColor::TextSecondary))
    .child(Text::new(&ctx.t(key)).variant(TypographyStyle::Caption))
    .into()
}

pub fn workflow_select(ctx: &Ctx, value: Signal<WorkflowChoice>, id: &'static str, enabled: bool) -> Element {
  Element::from(
    Select::new(value)
      .focusable(enabled)
      .options(WorkflowStatus::ALL.into_iter().map(|status| {
        lurq::components::SelectOption::new(WorkflowChoice(status), ctx.t(workflow_copy(status).1)).disabled(!enabled)
      }))
      .height(Dimension::Px(CONTROL_HEIGHT))
      .width(Dimension::Pct(100.0))
      .style(select_style(ctx, CONTROL_HEIGHT)),
  )
  .id(id)
}

pub fn priority_select(ctx: &Ctx, value: Signal<PriorityChoice>, id: &'static str, enabled: bool) -> Element {
  Element::from(
    Select::new(value)
      .focusable(enabled)
      .options(Priority::ALL.into_iter().map(|priority| {
        lurq::components::SelectOption::new(PriorityChoice(priority), ctx.t(priority_copy(priority).1))
          .disabled(!enabled)
      }))
      .height(Dimension::Px(CONTROL_HEIGHT))
      .width(Dimension::Pct(100.0))
      .style(select_style(ctx, CONTROL_HEIGHT)),
  )
  .id(id)
}

pub fn tag_chips(names: &[Arc<str>]) -> Element {
  Row::new()
    .spacing(SpacingSize::Xs)
    .with_children(names.iter().map(|name| {
      Row::new()
        .padding_horizontal(SpacingSize::Sm)
        .padding_vertical(ExtraSpacing::Indicator.role())
        .background(PaletteColor::SurfaceRaised)
        .rounded(lurq::app::theme::RadiusSize::Sm)
        .border_inside(lurq::app::theme::BorderSize::Sm, PaletteColor::Border)
        .child(
          Text::new(name)
            .variant(TypographyStyle::Caption)
            .color(PaletteColor::TextPrimary),
        )
    }))
    .into()
}
