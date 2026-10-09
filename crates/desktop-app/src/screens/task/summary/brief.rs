//! What the task asks for (`Brief` `Gt4az`, `Acceptance criteria` `ufoA8` in `lUXff`): the brief, and
//! the criteria as a numbered list with their count. A part the task does not have yet is left out.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::task::TaskDetails;

use super::section_head;
use crate::theme::{ExtraSpacing, TaskText};

/// The width of a criterion's number (`Os6zg`).
const NUMBER_WIDTH: f32 = 14.0;

pub fn sections(ctx: &mut Ctx, details: &TaskDetails) -> Vec<Element> {
  let brief = (!details.brief.trim().is_empty()).then(|| {
    Column::new()
      .id("task-brief")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(section_head(ctx, "task-brief-head", "summary.brief", None))
      .child(
        Text::new(&details.brief)
          .id("task-brief-text")
          .variant(TaskText::Brief.role())
          .width(Dimension::Pct(100.0))
          .selectable(true),
      )
      .into()
  });
  let criteria = &details.acceptance_criteria;
  let list = (!criteria.is_empty()).then(|| {
    Column::new()
      .id("task-criteria")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(section_head(ctx, "task-criteria-head", "summary.criteria", Some(criteria.len())))
      .with_children((1..).zip(criteria).map(|(number, text)| criterion(number, text)))
      .into()
  });
  brief.into_iter().chain(list).collect()
}

fn criterion(number: usize, text: &str) -> Element {
  Row::new()
    .id(format!("task-criterion-{number}"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Start)
    .child(
      Text::new(&number.to_string())
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .width(Dimension::Px(NUMBER_WIDTH)),
    )
    .child(
      Text::new(text)
        .variant(TypographyStyle::Body)
        .flex(1.0)
        .selectable(true),
    )
    .into()
}
