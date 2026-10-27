//! The Pipelines pages' frame (`Pipelines · redesign` `evQ4U`): the page, its header's back link and title line,
//! the body under the header with its pane or its flow and step column, and a step page's head.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::{ExtraSpacing, ExtraTypography, GroupText, IconSize, PipelinesSpacing},
  ui::{
    icon::{Icon, icon},
    pressable::{Press, pressed},
  },
};

/// A list or form pane's width at most (`Pane` `ZLmFa`).
pub const PANE_WIDTH: f32 = 720.0;

pub fn page() -> Column {
  Column::new()
    .id("screen-pipelines")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
}

/// The header's column: 20 over it, 28 at its sides, 10 between its lines (`Header` `Jf1Gu`).
pub fn header() -> Column {
  Column::new()
    .id("pipeline-header")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(PipelinesSpacing::PageTop.role())
        .left(ExtraSpacing::Roomy.role())
        .right(ExtraSpacing::Roomy.role()),
    )
    .spacing(ExtraSpacing::Loose.role())
}

/// The title line: the title, then what sits at its end (`Title row` `M8v20`).
pub fn title_row(id: &str, title: &str) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(
      Text::new(title)
        .id(id.to_owned())
        .variant(TypographyStyle::Heading)
        .nowrap()
        .text_overflow(lurq::components::TextOverflow::Elipsis)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}

/// `← <label>` (`Back` `MYia7`): back to where the page belongs.
pub fn back_link(id: &str, label: &str, press: Press) -> Element {
  pressed(press)
    .id(id.to_owned())
    .spacing(ExtraSpacing::Snug.role())
    .describe("a11y_name", label.to_owned())
    .child(icon(Icon::ArrowLeft, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(label)
        .id(format!("{id}-label"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted),
    )
    .into()
}

/// The body under the header, scrolling: 20 over it and 28 at its sides (`Body` `mifhW`).
pub fn body(id: &str, content: impl Into<Element>) -> ScrollVertical {
  ScrollVertical::new(
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(PipelinesSpacing::PageTop.role())
          .left(ExtraSpacing::Roomy.role())
          .right(ExtraSpacing::Roomy.role())
          .bottom(SpacingSize::Xl),
      )
      .child(content),
  )
  .id(id.to_owned())
  .width(Dimension::Pct(100.0))
  .flex(1.0)
  .min_height(Dimension::Px(0.0))
}

/// A pane of groups, 24 apart, [`PANE_WIDTH`] wide at most (`Pane` `ZLmFa`).
pub fn pane(id: &str) -> Column {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .max_width(Dimension::Px(PANE_WIDTH))
    .spacing(SpacingSize::Xl)
}

/// A step page's head: back to its step, its title and what it says of the step, on a line as high as a button
/// (`Sub-page head` `Z1LPH`).
pub fn sub_head(id: &str, back: Element, title: &str, meta: Option<&str>) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(back)
    .child(
      Row::new()
        .id(id.to_owned())
        .min_height(Dimension::Px(crate::theme::ControlHeight::Md.value()))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(
          Text::new(title)
            .id(format!("{id}-text"))
            .variant(TypographyStyle::Title),
        )
        .with_children(meta.map(|meta| Text::new(meta).id(format!("{id}-meta")).variant(GroupText::Hint.role()))),
    )
}

/// The words under a group or a page's head in 12.5 `text_secondary` (`XbZuU`).
pub fn lead(id: &str, words: &str) -> Text {
  Text::new(words)
    .id(id.to_owned())
    .variant(ExtraTypography::Lead.role())
    .width(Dimension::Pct(100.0))
}

pub fn caption(text: &str) -> Text {
  Text::new(text).variant(TypographyStyle::Caption)
}

pub fn note(text: &str) -> Text {
  Text::new(text)
    .variant(ExtraTypography::Note.role())
    .width(Dimension::Pct(100.0))
}

/// A change of a version, with its own symbol, as a row of a group (`Reha1` in `fXnX2`).
pub fn change_row(
  ctx: &Ctx,
  id: &str,
  change: &orchester_core::resource::PipelineChange,
) -> crate::ui::row_group::GroupRow {
  use orchester_core::resource::{PipelineChange, StepChange};
  let glyph = match change {
    PipelineChange::Description => Icon::FilePen,
    PipelineChange::Deliver(_) => Icon::GitMerge,
    PipelineChange::Delivers { .. } => Icon::FileText,
    PipelineChange::Step { change, .. } => match change {
      StepChange::Added => Icon::Plus,
      StepChange::Removed => Icon::Minus,
      StepChange::FailureRoute | StepChange::MaxRetries { .. } => Icon::Undo2,
      StepChange::Model | StepChange::Executor => Icon::Bot,
      StepChange::ApprovalGate { .. } => Icon::Hand,
      StepChange::Permissions | StepChange::RepositoryAccess => Icon::Shield,
      _ => Icon::FilePen,
    },
  };
  let words = crate::ui::message::message_text(ctx, &orchester_workspace::pipelines::describe::change(change));
  crate::ui::row_group::GroupRow::new(id, words)
    .marked()
    .leading(icon(glyph, IconSize::Md, PaletteColor::TextMuted))
}

pub fn pipeline_name_error(ctx: &Ctx, error: &orchester_workspace::messages::Message) -> std::sync::Arc<str> {
  if *error == orchester_workspace::messages::name_error(&orchester_core::name::NameError::Blank) {
    ctx.t("pipelines.invalidName")
  } else {
    crate::ui::message::message_text(ctx, error)
  }
}
