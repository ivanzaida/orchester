use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{FormButtonRole, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Select, Spacer, Text, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{Element, SelectPartStyle, SelectStyle, dimension::Dimension},
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
// gNExS/Sd5qt: compact header picker buttons.
const HEADER_HEIGHT: f32 = 36.0;
// gNExS/Sd5qt: both menus are 280 logical pixels wide.
pub const ORGANIZATION_MENU_WIDTH: f32 = 280.0;
/// A picker as a form field (`ixENv`), or as a secondary button that hugs its label: in the task
/// header (`gNExS`, `Sd5qt`) and for the task list's sort (`SoOX5` in `qVMJh`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PickerAppearance {
  Field,
  Header,
}
impl PickerAppearance {
  fn width(self) -> Dimension {
    match self {
      Self::Field => Dimension::Pct(100.0),
      Self::Header => Dimension::Auto,
    }
  }
  pub fn height(self) -> f32 {
    match self {
      Self::Field => CONTROL_HEIGHT,
      Self::Header => HEADER_HEIGHT,
    }
  }
  pub fn style(self, ctx: &Ctx) -> SelectStyle {
    let style = select_style(ctx, self.height())
      .min_menu_width(ORGANIZATION_MENU_WIDTH)
      .option_selected(SelectPartStyle::new().background(PaletteColor::AccentMuted));
    match self {
      Self::Field => style,
      Self::Header => style.trigger(
        SelectPartStyle::new()
          .background(PaletteColor::SurfaceRaised)
          .border_inside(lurq::app::theme::BorderSize::Sm, PaletteColor::Border)
          .rounded(lurq::app::theme::RadiusSize::Md)
          .min_height(HEADER_HEIGHT),
      ),
    }
  }
}

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

pub fn workflow_select(
  ctx: &Ctx,
  value: Signal<WorkflowChoice>,
  id: &'static str,
  enabled: bool,
  appearance: PickerAppearance,
) -> Element {
  Element::from(
    Select::new(value.clone())
      .focusable(enabled)
      .options(WorkflowStatus::ALL.into_iter().map(|status| {
        lurq::components::SelectOption::new(WorkflowChoice(status), ctx.t(workflow_copy(status).1)).disabled(!enabled)
      }))
      .trigger({
        let button = secondary_button(ctx);
        move |state| {
          select_trigger(
            workflow_copy(value.get().0).0,
            state.label.as_deref().unwrap_or_default(),
            appearance,
            &button,
          )
        }
      })
      .height(Dimension::Px(appearance.height()))
      .width(appearance.width())
      .style(appearance.style(ctx)),
  )
  .id(id)
}

pub fn priority_select(
  ctx: &Ctx,
  value: Signal<PriorityChoice>,
  id: &'static str,
  enabled: bool,
  appearance: PickerAppearance,
) -> Element {
  Element::from(
    Select::new(value.clone())
      .focusable(enabled)
      .options(Priority::ALL.into_iter().map(|priority| {
        lurq::components::SelectOption::new(PriorityChoice(priority), ctx.t(priority_copy(priority).1))
          .disabled(!enabled)
      }))
      .trigger({
        let button = secondary_button(ctx);
        move |state| {
          select_trigger(
            priority_copy(value.get().0).0,
            state.label.as_deref().unwrap_or_default(),
            appearance,
            &button,
          )
        }
      })
      .height(Dimension::Px(appearance.height()))
      .width(appearance.width())
      .style(appearance.style(ctx)),
  )
  .id(id)
}

/// The secondary button's role, which a header picker's trigger takes its look from.
pub fn secondary_button(ctx: &Ctx) -> FormButtonRole {
  ctx.theme().form().button.secondary.clone()
}

fn select_trigger(glyph: Icon, label: &str, appearance: PickerAppearance, button: &FormButtonRole) -> Element {
  match appearance {
    PickerAppearance::Field => Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .padding_horizontal(SpacingSize::Sm)
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(glyph, IconSize::Small, PaletteColor::TextSecondary))
      .child(Text::new(label).variant(TypographyStyle::Body))
      .child(Spacer::new().flex(1.0))
      .child(icon(Icon::ChevronDown, IconSize::Small, PaletteColor::TextSecondary))
      .into(),
    PickerAppearance::Header => button_trigger(glyph, label, button),
  }
}

/// A select trigger drawn as the secondary button (`u1XBP`): its icon and label, padded as the
/// button is.
pub fn button_trigger(glyph: Icon, label: &str, button: &FormButtonRole) -> Element {
  Row::new()
    .height(Dimension::Pct(100.0))
    .padding_custom(button.padding)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Button, button.text.color.clone()))
    .child(
      Text::new(label)
        .variant(button.text.typography)
        .color(button.text.color.clone())
        .nowrap(),
    )
    .into()
}

pub fn tag_chips(names: &[Arc<str>]) -> Element {
  Row::new()
    .wrap()
    .spacing(SpacingSize::Xs)
    .with_children(names.iter().map(|name| tag_chip(name)))
    .into()
}

pub fn tag_chip(name: &str) -> Element {
  Row::new()
    .max_width(Dimension::Px(TAG_MAX_WIDTH))
    .padding_horizontal(SpacingSize::Sm)
    .padding_vertical(ExtraSpacing::Tight.role())
    .background(PaletteColor::SurfaceRaised)
    .rounded(lurq::app::theme::RadiusSize::Sm)
    .border_inside(lurq::app::theme::BorderSize::Sm, PaletteColor::Border)
    .child(
      Text::new(name)
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextPrimary)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .into()
}

// yrT73: compact row overflow, wrapped form/header chips, and long tag names.
const TAG_MAX_WIDTH: f32 = 160.0;
