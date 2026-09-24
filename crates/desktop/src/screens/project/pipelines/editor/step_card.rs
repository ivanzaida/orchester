//! Pipeline step card (`ajikA`): the step's icon, name and number, who runs it and what it
//! waits for. The selected card has a stronger outline; a step without a profile or tool says
//! so in the text color (`ipfll` in `UjHDL`). A card is a button: Tab reaches it and Enter or
//! Space selects its step.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Style, border::Border, dimension::Dimension},
};
use orchester_core::resource::{RepositoryAccess, Step, StepKind};

use super::{EditorHandle, Selection};
use crate::{
  pipelines::{
    data::Assignables,
    describe::{executor_line, outcome_line},
  },
  theme::ExtraSpacing,
  ui::{
    icon::{Icon, IconSize, icon},
    message_text,
    pressable::{Ring, pressable, surface},
  },
};

/// The icon of a step: what kind of work it does. Steps have no icon of their own, so the
/// design's per-step icons are approximated from the step's kind.
pub fn step_icon(step: &Step) -> Icon {
  match &step.kind {
    StepKind::Tool { .. } => Icon::Terminal,
    StepKind::Agent { .. } if step.approval_gate => Icon::ListChecks,
    StepKind::Agent {
      repository: RepositoryAccess::Write,
      ..
    } => Icon::Code,
    StepKind::Agent {
      repository: RepositoryAccess::ReadOnly,
      ..
    } => Icon::Search,
  }
}

pub fn step_card(
  ctx: &Ctx,
  step: &Step,
  number: usize,
  selected: bool,
  assignables: &Assignables,
  handle: &EditorHandle,
) -> Button {
  let unassigned = step.kind.executor().is_none();
  let header = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .child(icon(step_icon(step), IconSize::Notice, PaletteColor::TextPrimary))
    .child(
      Text::new(step.name.as_str())
        .variant(TypographyStyle::Label)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex(1.0),
    )
    .child(
      Text::new(&format!("{number:02}"))
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextMuted),
    );
  let executor_color = if unassigned {
    PaletteColor::TextPrimary
  } else {
    PaletteColor::TextSecondary
  };
  let (stroke, outline) = if selected {
    (BorderSize::Md, PaletteColor::TextPrimary)
  } else {
    (BorderSize::Sm, PaletteColor::Border)
  };
  let (select, key) = (handle.clone(), step.key.clone());
  let content = Column::new()
    .flex(1.0)
    .spacing(ExtraSpacing::Stack.role())
    .child(header)
    .child(
      Text::new(&message_text(ctx, &executor_line(step, assignables)))
        .variant(TypographyStyle::Caption)
        .color(executor_color)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .child(
      Text::new(&message_text(ctx, &outcome_line(step)))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextMuted)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    );
  let card = surface()
    .id(format!("step-card-{}", step.key))
    .describe("selected", if selected { "true" } else { "false" })
    .padding(SpacingSize::Md)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(stroke, outline))
    .rounded(RadiusSize::Lg)
    .child(content);
  pressable(card, Ring::Outer, move || select.select(Selection::Step(key.clone())))
    .hovered_style(Style::new().border(Border::inside(stroke, PaletteColor::BorderStrong)))
}
